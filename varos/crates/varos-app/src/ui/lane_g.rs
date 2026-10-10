//! Lane G: shared frame contract for Home, documents and presentation.
use super::*;
pub(super) fn frame(
    ctx: &egui::Context,
    input: egui::RawInput,
    release: &mut crate::release_ui::State,
    mut draw: impl FnMut(&mut egui::Ui),
) -> (egui::FullOutput, Vec<AppCommand>) {
    let mut commands = vec![];
    let out = ctx.run_ui(input, |root| {
        draw(root);
        release.draw(root.ctx(), &mut commands);
    });
    (out, commands)
}
fn publish_output(output: &egui::PlatformOutput, publish: impl FnOnce(&accesskit::TreeUpdate)) {
    if let Some(tree) = &output.accesskit_update {
        publish(tree);
    }
}
pub(super) fn platform(
    state: &mut egui_winit::State,
    window: &Window,
    ctx: &egui::Context,
    output: egui::PlatformOutput,
) {
    publish_output(&output, |tree| {
        #[cfg(target_os = "macos")]
        varos_app::accessibility_macos::publish(window, ctx, tree);
        #[cfg(not(target_os = "macos"))]
        let _ = (ctx, tree);
    });
    state.handle_platform_output(window, output);
}

#[cfg(test)]
mod tests {
    use super::*;
    use varos_bridge::release::Sheet;
    fn run_frame(
        ctx: &egui::Context,
        input: egui::RawInput,
        release: &mut crate::release_ui::State,
        commands: &mut Vec<AppCommand>,
        mut draw: impl FnMut(&mut egui::Ui, &mut Vec<AppCommand>),
        publish: impl FnOnce(&accesskit::TreeUpdate),
    ) -> egui::FullOutput {
        let (out, release_cmds) = frame(ctx, input, release, |root| draw(root, commands));
        commands.extend(release_cmds);
        publish_output(&out.platform_output, publish);
        out
    }
    #[test]
    fn each_mode_publishes_current_controls_and_release_sheets() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        varos_app::shell::fonts::install(&ctx);
        let mut release = crate::release_ui::State::default();
        let mut commands = vec![];
        let id = egui::Id::new("mode-control");
        for name in ["Home", "Document", "Home", "Presentation"] {
            let mut published = None;
            let _ = run_frame(
                &ctx,
                Default::default(),
                &mut release,
                &mut commands,
                |root, _| {
                    let response = root.interact(
                        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100., 30.)),
                        id,
                        egui::Sense::click(),
                    );
                    varos_app::shell::accessibility::emit(root, response.id, accesskit::Role::Button, name, None, true);
                },
                |tree| published = Some(tree.clone()),
            );
            let tree = published.expect("every mode publishes a tree");
            assert!(tree.nodes.iter().any(|(_, n)| n.label() == Some(name)));
            assert!(!tree
                .nodes
                .iter()
                .any(|(_, n)| n.label() == Some(if name == "Document" { "Home" } else { "Document" })));
            if name == "Home" {
                let target = tree.nodes.iter().find(|(_, n)| n.label() == Some(name)).unwrap().0;
                let input = egui::RawInput {
                    events: vec![egui::Event::AccessKitActionRequest(accesskit::ActionRequest {
                        action: accesskit::Action::Click,
                        target_tree: accesskit::TreeId::ROOT,
                        target_node: target,
                        data: None,
                    })],
                    ..Default::default()
                };
                let mut clicked = false;
                let _ = run_frame(
                    &ctx,
                    input,
                    &mut release,
                    &mut commands,
                    |root, _| {
                        clicked |= root
                            .interact(
                                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100., 30.)),
                                id,
                                egui::Sense::click(),
                            )
                            .clicked();
                    },
                    |_| {},
                );
                assert!(clicked, "Home consumes assistive activation");
            }
            for sheet in [Sheet::Failed("Offline".into()), Sheet::Crash("Crash evidence".into())] {
                let crash = matches!(sheet, Sheet::Crash(_));
                release.sheet = Some(sheet);
                let mut labels = vec![];
                let _ = run_frame(
                    &ctx,
                    Default::default(),
                    &mut release,
                    &mut commands,
                    |_, _| {},
                    |tree| {
                        labels = tree.nodes.iter().filter_map(|(_, n)| n.label().map(str::to_owned)).collect();
                    },
                );
                assert!(
                    labels.iter().any(|label| label == if crash { "Crash Report" } else { "Check for Updates" }),
                    "{name}: {labels:?}"
                );
            }
            release.sheet = None;
        }
        assert!(commands.is_empty());
    }
    #[test]
    fn completion_is_consumed_and_visible_without_a_document() {
        for mode in ["Home", "Document", "Presentation"] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            varos_app::shell::fonts::install(&ctx);
            let mut release =
                crate::release_ui::State::completed_for_test(Sheet::Failed("Completed offline check".into()));
            let (out, commands) = frame(&ctx, Default::default(), &mut release, |_| {});
            assert!(matches!(release.sheet, Some(Sheet::Failed(_))), "{mode}");
            let mut names = vec![];
            publish_output(&out.platform_output, |tree| {
                names = tree.nodes.iter().filter_map(|(_, n)| n.label().map(str::to_owned)).collect()
            });
            assert!(names.iter().any(|name| name == "Completed offline check"), "{mode}: {names:?}");
            assert!(commands.is_empty());
        }
    }
    #[test]
    fn mode_renderers_cannot_skip_the_shared_contract() {
        for source in [include_str!("../ui.rs"), include_str!("home.rs"), include_str!("view_modes.rs")] {
            assert!(source.contains("lane_g::frame"));
            assert!(source.contains("lane_g::platform"));
            assert!(!source.contains("self.ctx.run_ui"));
        }
        let source = include_str!("../ui.rs");
        assert!(source.find("accessibility_macos::drain").unwrap() < source.find("if self.home {").unwrap());
    }
}
