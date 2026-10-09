//! Retag only the in-memory header of a frozen future fixture. Its disk bytes remain unchanged.
//! The historic fixture's era-9 gate is covered separately; a current-reader gate must use >current.
pub fn promote(bytes: &[u8]) -> Vec<u8> {
    let next = varos_core::format::FORMAT_VERSION + 1;
    fn replace(bytes: Vec<u8>, from: &[u8], to: &[u8]) -> Vec<u8> {
        // The reserved wave-3 versions remain two digits, keeping frozen PDF xref offsets valid.
        assert_eq!(from.len(), to.len());
        let mut out = Vec::new();
        let mut at = 0;
        while at < bytes.len() {
            if bytes[at..].starts_with(from) {
                out.extend_from_slice(to);
                at += from.len();
            } else {
                out.push(bytes[at]);
                at += 1;
            }
        }
        out
    }
    let bytes = replace(bytes.to_vec(), b"\"varos\":10", format!("\"varos\":{next}").as_bytes());
    replace(bytes, b"/VAROS_SchemaVersion 10", format!("/VAROS_SchemaVersion {next}").as_bytes())
}
