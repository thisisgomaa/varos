//! Lane G: NSAccessibility adapter on winit's NSView; accesskit_winit is unavailable offline.
//! Kit emits AccessKit semantics. Native actions return through egui's activation/focus path.
use accesskit::{Action, ActionRequest, NodeId, Role, TreeId, TreeUpdate};
use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
    DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityElement, NSAccessibilityFocusedUIElementChangedNotification,
    NSAccessibilityLayoutChangedNotification, NSAccessibilityPostNotification,
};
use objc2_foundation::{NSArray, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{cell::RefCell, collections::BTreeMap};
thread_local! { static ACTIONS: RefCell<Vec<ActionRequest>> = const { RefCell::new(Vec::new()) }; }
struct Ivars {
    id: NodeId,
    ctx: egui::Context,
    clickable: bool,
    enabled: bool,
    focused: bool,
}
type Snapshot = (usize, TreeUpdate, Option<winit::dpi::PhysicalPosition<i32>>);
thread_local! { static LAST: RefCell<Option<Snapshot>> = const { RefCell::new(None) }; }
define_class!(
    #[unsafe(super = NSAccessibilityElement)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    #[name = "VarosKitAccessibilityElement"]
    struct Element;
    unsafe impl NSObjectProtocol for Element {}
    impl Element {
        #[unsafe(method(accessibilityPerformPress))]
        fn press(&self) -> bool {
            if !self.ivars().clickable || !self.ivars().enabled {false} else { self.enqueue(Action::Click); true }
        }
        #[unsafe(method(isAccessibilityFocused))]
        fn focused(&self) -> bool { self.ivars().focused }
        #[unsafe(method(setAccessibilityFocused:))]
        fn focus(&self, focused: bool) { if focused && self.ivars().enabled {self.enqueue(Action::Focus);} }
    }
);
impl Element {
    fn enqueue(&self, action: Action) {
        ACTIONS.with(|q| {
            q.borrow_mut().push(ActionRequest {
                action,
                target_tree: TreeId::ROOT,
                target_node: self.ivars().id,
                data: None,
            })
        });
        self.ivars().ctx.request_repaint();
    }
    fn new(mtm: MainThreadMarker, ivars: Ivars) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ivars);
        // SAFETY: inherited NSObject init, allocated on the main thread.
        unsafe { msg_send![super(this), init] }
    }
}
pub fn drain(events: &mut Vec<egui::Event>) {
    ACTIONS.with(|q| events.extend(q.borrow_mut().drain(..).map(egui::Event::AccessKitActionRequest)));
}
fn role(role: Role) -> &'static str {
    match role {
        Role::Button => "AXButton",
        Role::CheckBox | Role::Switch => "AXCheckBox",
        Role::RadioButton | Role::Tab => "AXRadioButton",
        Role::TabList => "AXTabGroup",
        Role::TextInput | Role::MultilineTextInput | Role::SpinButton => "AXTextField",
        Role::ComboBox => "AXPopUpButton",
        Role::MenuItem => "AXMenuItem",
        Role::Menu => "AXMenu",
        Role::List | Role::ListBox => "AXList",
        Role::ListBoxOption => "AXRow",
        Role::Label | Role::Heading => "AXStaticText",
        _ => "AXGroup",
    }
}
fn toggle_value(value: accesskit::Toggled) -> Option<Retained<AnyObject>> {
    let class = AnyClass::get(c"NSNumber")?;
    let value: i32 = match value {
        accesskit::Toggled::False => 0,
        accesskit::Toggled::True => 1,
        accesskit::Toggled::Mixed => 2,
    };
    // SAFETY: Foundation's NSNumber factory accepts an int and returns a retained number.
    Some(unsafe { msg_send![class, numberWithInt: value] })
}
/// winit's NSView is flipped: AppKit performs the screen-space conversion itself.
fn view_bounds(bounds: accesskit::Rect) -> NSRect {
    NSRect::new(NSPoint::new(bounds.x0, bounds.y0), NSSize::new(bounds.width(), bounds.height()))
}
fn apply_selection(node: &accesskit::Node, set: impl FnOnce(bool)) {
    if let Some(selected) = node.is_selected() {
        set(selected);
    }
}
fn native_value(node: &accesskit::Node) -> Option<Retained<AnyObject>> {
    if matches!(node.role(), Role::Tab | Role::RadioButton) {
        if let Some(selected) = node.is_selected() {
            return toggle_value(if selected { accesskit::Toggled::True } else { accesskit::Toggled::False });
        }
    }
    if let Some(value) = node.toggled().and_then(toggle_value) {
        return Some(value);
    }
    node.value().map(|value| NSString::from_str(value).into_super().into_super())
}
pub fn publish(window: &winit::window::Window, ctx: &egui::Context, tree: &TreeUpdate) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Ok(handle) = window.window_handle() else { return };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else { return };
    // SAFETY: winit owns this live NSView; used only on the main thread.
    let view = unsafe { &*handle.ns_view.as_ptr().cast::<objc2_app_kit::NSView>() };
    let identity = handle.ns_view.as_ptr() as usize;
    let origin = window.outer_position().ok();
    if LAST.with(|last| last.borrow().as_ref().is_some_and(|(old, t, p)| *old == identity && t == tree && *p == origin))
    {
        return;
    }
    let focus_changed = LAST.with(|last| last.borrow().as_ref().is_none_or(|(_, old, _)| old.focus != tree.focus));
    LAST.with(|last| *last.borrow_mut() = Some((identity, tree.clone(), origin)));
    let mut elements = BTreeMap::new();
    for (id, node) in &tree.nodes {
        let element = Element::new(
            mtm,
            Ivars {
                id: *id,
                ctx: ctx.clone(),
                clickable: node.supports_action(Action::Click),
                enabled: !node.is_disabled(),
                focused: tree.focus == *id,
            },
        );
        let native: &NSAccessibilityElement = &element;
        native.setAccessibilityElement(node.label().is_some() || node.supports_action(Action::Click));
        native.setAccessibilityRole(Some(&NSString::from_str(role(node.role()))));
        if let Some(label) = node.label() {
            native.setAccessibilityLabel(Some(&NSString::from_str(label)));
        }
        apply_selection(node, |selected| native.setAccessibilitySelected(selected));
        if let Some(value) = native_value(node) {
            // SAFETY: NSNumber and NSString are supported NSAccessibility value objects.
            unsafe {
                native.setAccessibilityValue(Some(&value));
            }
        }
        native.setAccessibilityEnabled(!node.is_disabled());
        if let Some(bounds) = node.bounds() {
            let frame = view_bounds(bounds);
            if let Some(window) = view.window() {
                let window_frame = view.convertRect_toView(frame, None);
                native.setAccessibilityFrame(window.convertRectToScreen(window_frame));
            }
        }
        elements.insert(*id, element);
    }
    for (id, node) in &tree.nodes {
        let Some(parent) = elements.get(id) else { continue };
        let native: &NSAccessibilityElement = parent;
        for child in node.children() {
            if let Some(child) = elements.get(child) {
                let child_native: &NSAccessibilityElement = child;
                native.accessibilityAddChildElement(child_native);
                // SAFETY: the parent is a live retained accessibility element.
                unsafe {
                    child_native.setAccessibilityParent(Some(parent));
                }
            }
        }
    }
    if let Some(root) = tree.tree.as_ref().and_then(|root| elements.get(&root.root)) {
        let root_native: &NSAccessibilityElement = root;
        // SAFETY: winit owns the live NSView parent for the lifetime of this hierarchy.
        unsafe {
            root_native.setAccessibilityParent(Some(view));
        }
        let object: &AnyObject = root;
        let roots = NSArray::from_slice(&[object]);
        // SAFETY: children are NSObject-derived elements retained by the array/view.
        unsafe {
            view.setAccessibilityChildren(Some(&roots));
            view.setAccessibilityElement(false);
            // AppKit must invalidate its cached hierarchy when the kit changes.
            NSAccessibilityPostNotification(view, NSAccessibilityLayoutChangedNotification);
            if focus_changed {
                if let Some(focused) = elements.get(&tree.focus) {
                    NSAccessibilityPostNotification(focused, NSAccessibilityFocusedUIElementChangedNotification);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flipped_view_bounds_preserve_top_left() {
        let frame = view_bounds(accesskit::Rect::new(12., 24., 112., 54.));
        assert_eq!(frame.origin, NSPoint::new(12., 24.));
        assert_eq!(frame.size, NSSize::new(100., 30.));
    }
    #[test]
    fn native_selection_transfers_both_states_for_tabs_and_list_rows() {
        for role in [Role::Tab, Role::ListBoxOption] {
            for selected in [true, false] {
                let mut node = accesskit::Node::new(role);
                node.set_selected(selected);
                let mut native_selected = None;
                apply_selection(&node, |value| native_selected = Some(value));
                assert_eq!(native_selected, Some(selected));
            }
        }
        apply_selection(&accesskit::Node::new(Role::Button), |_| panic!("no selected property"));
    }
    #[test]
    fn selected_tabs_use_numeric_values_instead_of_counts() {
        for selected in [false, true] {
            for role in [Role::Tab, Role::RadioButton] {
                let mut node = accesskit::Node::new(role);
                node.set_selected(selected);
                node.set_value("42");
                let value = native_value(&node).unwrap();
                // SAFETY: selected tabs/radios map to NSNumber.
                let actual: i32 = unsafe { msg_send![&*value, intValue] };
                assert_eq!(actual, i32::from(selected));
            }
        }
    }
    #[test]
    fn toggle_values_are_native_numbers() {
        for (toggle, expected) in
            [(accesskit::Toggled::False, 0i32), (accesskit::Toggled::True, 1), (accesskit::Toggled::Mixed, 2)]
        {
            let value = toggle_value(toggle).expect("Foundation NSNumber is available");
            // SAFETY: toggle_value returns an NSNumber, whose intValue is an int.
            let actual: i32 = unsafe { msg_send![&*value, intValue] };
            assert_eq!(actual, expected);
            let class = AnyClass::get(c"NSNumber").expect("NSNumber class");
            // SAFETY: every NSObject supports isKindOfClass: with a valid class.
            let is_number: bool = unsafe { msg_send![&*value, isKindOfClass: class] };
            assert!(is_number);
        }
    }
}
