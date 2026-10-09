use serde_json::json;
use std::sync::atomic::AtomicBool;
use varos_bridge::{BoardAccess, BoardInfo, Context, Error, Host, Reply, Service};
use varos_core::{Editor, ExportNote, ExportReport};

struct ExportHost {
    editor: Editor,
    report: ExportReport,
    completed: bool,
}
impl Host for ExportHost {
    fn boards(&self) -> Vec<BoardInfo> {
        vec![BoardInfo {
            board: "b1".into(),
            name: "Report".into(),
            rev: self.editor.rev,
            dirty: false,
            active: true,
            backing_file: None,
        }]
    }
    fn prepare(&mut self, _: &str, _: bool) -> Result<(), Error> {
        Ok(())
    }
    fn access(&mut self, _: &str) -> Result<BoardAccess<'_>, Error> {
        Ok(BoardAccess { editor: &mut self.editor, dirty: false })
    }
    fn file_effect(&mut self, _: &str, _: &varos_bridge::dto::FileEffect) -> Result<Reply, Error> {
        Ok(Reply::success(json!({"accepted":true,"ticket":17})))
    }
    fn file_pending(&self, _: u64) -> bool {
        true
    }
    fn file_status(&mut self, ticket: u64) -> Option<Reply> {
        (ticket == 17 && self.completed)
            .then(|| Reply::success(json!({"exported":true,"durable":true,"report":self.report})))
    }
}
#[test]
fn worker_report_round_trips_through_bridge_12_and_stays_out_of_legacy_receipts() {
    let report = ExportReport {
        notes: vec![ExportNote {
            kind: "synthetic".into(),
            object_id: Some(42),
            message: "A worker diagnostic".into(),
        }],
    };
    for api in ["1.0", "1.1", "1.2"] {
        let mut host = ExportHost { editor: Editor::new(), report: report.clone(), completed: false };
        let mut service = Service::new("test".into());
        let context = Context { epoch: "test".into(), client: "export-test".into() };
        let cancel = AtomicBool::new(false);
        let request = varos_bridge::mcp::decode_tool("export_pdf", json!({"api":api,"board":"b1","expected_rev":host.editor.rev,"request_id":"r1","path":"/granted/report.pdf","scope":"artwork_bounds"})).unwrap();
        let accepted = service.handle(&mut host, &context, request.clone(), &cancel);
        assert!(accepted.ok, "{accepted:?}");
        assert!(accepted.result.as_ref().unwrap().get("report").is_none());
        // Retried requests must retain both receipt identity and the report opt-in.
        assert_eq!(
            serde_json::to_vec(&accepted).unwrap(),
            serde_json::to_vec(&service.handle(&mut host, &context, request, &cancel)).unwrap()
        );
        let status = || varos_bridge::mcp::decode_tool("request_status", json!({"request_id":"r1"})).unwrap();
        let pending = service.handle(&mut host, &context, status(), &cancel);
        assert_eq!(pending.result.unwrap()["status"], "pending");
        host.completed = true;
        let completed = service.handle(&mut host, &context, status(), &cancel);
        assert!(completed.ok, "{completed:?}");
        let decoded: Reply = serde_json::from_slice(&serde_json::to_vec(&completed).unwrap()).unwrap();
        let receipt = &decoded.result.as_ref().unwrap()["receipt"]["result"];
        if api == "1.2" {
            assert_eq!(serde_json::from_value::<ExportReport>(receipt["report"].clone()).unwrap(), report);
        } else {
            assert_eq!(receipt, &json!({"exported":true,"durable":true}));
        }
    }
}
#[test]
fn only_export_pdf_advertises_api_12() {
    let value = varos_bridge::mcp::tools();
    for tool in value["tools"].as_array().unwrap() {
        let api = &tool["inputSchema"]["properties"]["api"];
        if let Some(values) = api["enum"].as_array() {
            assert_eq!(
                values.iter().any(|v| v == "1.2"),
                matches!(tool["name"].as_str(), Some("export_pdf" | "select" | "edit"))
            );
        }
    }
}
