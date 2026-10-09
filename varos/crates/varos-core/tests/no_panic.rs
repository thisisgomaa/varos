//! Non-test panic-site ratchet: count Rust calls, not comments or quoted strings.
use std::path::Path;
use syn::visit::{self, Visit};
fn test_only(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("test")
            || (a.path().is_ident("cfg") && a.parse_args::<syn::Path>().is_ok_and(|p| p.is_ident("test")))
    })
}
#[derive(Default)]
struct Counter(usize);
impl<'ast> Visit<'ast> for Counter {
    fn visit_item_mod(&mut self, n: &'ast syn::ItemMod) {
        if !test_only(&n.attrs) {
            visit::visit_item_mod(self, n);
        }
    }
    fn visit_item_fn(&mut self, n: &'ast syn::ItemFn) {
        if !test_only(&n.attrs) {
            visit::visit_item_fn(self, n);
        }
    }
    fn visit_impl_item_fn(&mut self, n: &'ast syn::ImplItemFn) {
        if !test_only(&n.attrs) {
            visit::visit_impl_item_fn(self, n);
        }
    }
    fn visit_expr_if(&mut self, n: &'ast syn::ExprIf) {
        if !test_only(&n.attrs) {
            visit::visit_expr_if(self, n);
        }
    }
    fn visit_arm(&mut self, n: &'ast syn::Arm) {
        if !test_only(&n.attrs) {
            visit::visit_arm(self, n);
        }
    }
    fn visit_expr_method_call(&mut self, n: &'ast syn::ExprMethodCall) {
        if n.method == "unwrap" || n.method == "expect" {
            self.0 += 1;
        }
        visit::visit_expr_method_call(self, n);
    }
    fn visit_macro(&mut self, n: &'ast syn::Macro) {
        if n.path
            .segments
            .last()
            .is_some_and(|s| matches!(s.ident.to_string().as_str(), "panic" | "unreachable" | "todo"))
        {
            self.0 += 1;
        }
        self.0 += token_calls(n.tokens.clone());
        visit::visit_macro(self, n);
    }
}
fn token_calls(tokens: proc_macro2::TokenStream) -> usize {
    use proc_macro2::{Delimiter, TokenTree as T};
    let tokens: Vec<_> = tokens.into_iter().collect();
    let mut total = 0;
    for (index, token) in tokens.iter().enumerate() {
        if let T::Group(g) = token {
            total += token_calls(g.stream());
        }
        if let T::Ident(id) = token {
            let method = matches!(id.to_string().as_str(), "unwrap" | "expect")
                && index > 0
                && matches!(&tokens[index-1], T::Punct(p) if p.as_char()=='.')
                && matches!(tokens.get(index+1),Some(T::Group(g)) if g.delimiter()==Delimiter::Parenthesis);
            let mac = matches!(id.to_string().as_str(), "panic" | "unreachable" | "todo")
                && matches!(tokens.get(index+1),Some(T::Punct(p)) if p.as_char()=='!');
            total += usize::from(method || mac);
        }
    }
    total
}
fn count(dir: &Path) -> usize {
    let mut total = 0;
    for entry in std::fs::read_dir(dir).expect("source directory") {
        let p = entry.expect("source entry").path();
        if p.is_dir() {
            total += count(&p);
        } else if p.extension().is_some_and(|e| e == "rs")
            && p.file_name().is_some_and(|n| n != "tests.rs" && !n.to_string_lossy().ends_with("_tests.rs"))
        {
            let text = std::fs::read_to_string(p).expect("source file");
            let mut counter = Counter::default();
            counter.visit_file(&syn::parse_file(&text).expect("Rust source"));
            total += counter.0;
        }
    }
    total
}
#[test]
fn non_test_panic_sites_never_increase() {
    let actual = count(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
    const BASELINE: usize = 37;
    println!("panic ratchet: {actual}/{BASELINE}");
    assert!(actual <= BASELINE, "panic sites {actual} exceed baseline {BASELINE}");
}
#[test]
fn ignores_test_modules_and_string_literals() {
    let src = r##"fn prod() { let _ = ".unwrap() panic!"; None::<u8>.unwrap(); } #[cfg(test)] mod tests { fn x() { panic!("test"); } }"##;
    let mut c = Counter::default();
    c.visit_file(&syn::parse_file(src).unwrap());
    assert_eq!(c.0, 1);
    c = Counter::default();
    c.visit_file(&syn::parse_file("fn x() { json!({\"x\": None::<u8>.expect(\"value\")}); }").unwrap());
    assert_eq!(c.0, 1);
}
