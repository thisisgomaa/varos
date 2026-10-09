//! Lane F: typed application/history discovery and two-agent review, headless only.
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use varos_bridge::{BoardAccess, BoardInfo, Context, Error, Host, Reply, Service};
use varos_core::{
    actions::{Actions, Step},
    editor::{history::Actor, Editor},
};
struct Fake(Editor);
impl Host for Fake {
    fn boards(&self) -> Vec<BoardInfo> {
        vec![BoardInfo {
            board: "b1".into(),
            name: "Test".into(),
            rev: self.0.rev,
            dirty: true,
            active: true,
            backing_file: None,
        }]
    }
    fn prepare(&mut self, board: &str, _: bool) -> Result<(), Error> {
        if board == "b1" {
            Ok(())
        } else {
            Err(Error::new("not_found", "closed"))
        }
    }
    fn access(&mut self, board: &str) -> Result<BoardAccess<'_>, Error> {
        self.prepare(board, false)?;
        Ok(BoardAccess { editor: &mut self.0, dirty: true })
    }
}
fn call(service: &mut Service, host: &mut Fake, client: &str, tool: &str, args: Value) -> Reply {
    service.handle(
        host,
        &Context { client: client.into(), epoch: "test".into() },
        varos_bridge::mcp::decode_tool(tool, args).unwrap(),
        &AtomicBool::new(false),
    )
}
fn actions() -> Actions {
    Actions {
        version: 1,
        name: "Create".into(),
        steps: vec![Step::Rectangle { local: "r".into(), bounds_pt: [0., 0., 10., 10.], fill: None }],
    }
}
#[test]
fn two_agents_history_jump_top_rule_and_idempotent_replay() {
    let mut host = Fake(Editor::new());
    let mut service = Service::new("test".into());
    let request = json!({"api":"1.2","board":"b1","request_id":"r1","expected_rev":0,"action":{"kind":"replay","actions":actions()}});
    assert!(call(&mut service, &mut host, "agent-a:connection", "actions", request.clone()).ok);
    assert!(call(&mut service, &mut host, "agent-a:connection", "actions", request).ok);
    assert_eq!(host.0.doc.paths.len(), 1);
    let rev = host.0.rev;
    assert!(call(&mut service, &mut host, "agent-b:connection", "actions", json!({"api":"1.2","board":"b1","request_id":"r1","expected_rev":rev,"action":{"kind":"replay","actions":actions()}})).ok);
    let rev = host.0.rev;
    let listing = call(
        &mut service,
        &mut host,
        "agent-a:connection",
        "history_list",
        json!({"api":"1.2","board":"b1","limit":1}),
    );
    let listing = listing.result.unwrap();
    assert_eq!(listing["entries"][0]["actor"]["profile_id"], "agent-b");
    assert_eq!(listing["next_cursor"], 1);
    assert!(
        !call(
            &mut service,
            &mut host,
            "agent-a:connection",
            "actions",
            json!({"api":"1.2","board":"b1","request_id":"r2","expected_rev":rev,"action":{"kind":"undo_mine"}})
        )
        .ok
    );
    assert_eq!(host.0.doc.paths.len(), 2);
    let undone = call(
        &mut service,
        &mut host,
        "agent-b:connection",
        "actions",
        json!({"api":"1.2","board":"b1","request_id":"r2","expected_rev":rev,"action":{"kind":"undo_mine"}}),
    );
    assert!(undone.ok);
    assert_eq!(undone.undo_steps, 0);
    assert_eq!(host.0.doc.paths.len(), 1);
    let rev = host.0.rev;
    assert!(
        call(
            &mut service,
            &mut host,
            "agent-a:connection",
            "history_jump",
            json!({"api":"1.2","board":"b1","request_id":"r3","expected_rev":rev,"undo_depth":2})
        )
        .ok
    );
    assert_eq!(host.0.doc.paths.len(), 2);
    assert!(matches!(&host.0.history_entries()[0].actor, Actor::Agent { profile_id, .. } if profile_id == "agent-a"));
}
#[test]
fn discovery_reports_unsupported_host_needs_arguments_and_schemas() {
    let mut host = Fake(Editor::new());
    let mut service = Service::new("test".into());
    let reply = call(&mut service, &mut host, "agent", "list_verbs", json!({"api":"1.2"}));
    let groups = reply.result.unwrap()["groups"].as_array().unwrap().clone();
    let rows = groups.iter().flat_map(|g| g["verbs"].as_array().unwrap()).collect::<Vec<_>>();
    for name in ["preferences", "shortcuts", "command_index"] {
        let row = rows.iter().find(|r| r["name"] == name).unwrap();
        assert_eq!(row["enabled"], false);
        assert_eq!(row["disabled_reason"], "unsupported_host");
        assert!(varos_bridge::mcp::schema(name, None).is_ok());
    }
    let nudge = rows.iter().find(|r| r["name"] == "move").unwrap();
    assert_eq!(nudge["id"], "edit.nudge");
    assert_eq!(nudge["disabled_reason"], "needs_arguments");
    assert!(!call(&mut service, &mut host, "agent", "preferences", json!({"api":"1.0","action":{"kind":"read"}})).ok);
    for api in ["1.0", "1.1"] {
        assert!(varos_bridge::mcp::tools_for(api)["tools"].as_array().unwrap().iter().all(|r| r["name"] != "actions"));
    }
}
#[test]
fn recording_controls_do_not_create_history_and_stale_replay_refuses() {
    let mut host = Fake(Editor::new());
    let mut service = Service::new("test".into());
    let start = call(
        &mut service,
        &mut host,
        "agent",
        "actions",
        json!({"api":"1.2","board":"b1","request_id":"r1","expected_rev":0,"action":{"kind":"start"}}),
    );
    assert!(start.ok);
    assert_eq!(start.undo_steps, 0);
    assert!(host.0.history_entries().is_empty());
    assert!(!call(&mut service, &mut host, "agent", "actions", json!({"api":"1.2","board":"b1","request_id":"r2","expected_rev":99,"action":{"kind":"replay","actions":actions()}})).ok);
    assert!(host.0.doc.paths.is_empty());
}

#[test]
fn bridge_committed_invocations_are_recorded_and_noop_leaves_are_omitted() {
    let mut host = Fake(Editor::new());
    actions().replay(&mut host.0).unwrap();
    let path = host.0.doc.paths[0].id;
    let id = format!("path:{path}");
    let mut service = Service::new("test".into());
    let rev = host.0.rev;
    assert!(
        call(
            &mut service,
            &mut host,
            "agent",
            "actions",
            json!({"api":"1.2","board":"b1","request_id":"r1","expected_rev":rev,"action":{"kind":"start"}})
        )
        .ok
    );
    assert!(call(&mut service, &mut host, "agent", "edit", json!({"api":"1.2","board":"b1","request_id":"r2","expected_rev":rev,"ops":[{"verb":"move","ids":[id],"delta":[0,0]},{"verb":"move","ids":[id],"delta":[2,3]},{"verb":"set_paint","ids":[id],"opacity":0.5}]})).ok);
    let rev = host.0.rev;
    let stopped = call(
        &mut service,
        &mut host,
        "agent",
        "actions",
        json!({"api":"1.2","board":"b1","request_id":"r3","expected_rev":rev,"action":{"kind":"stop","name":"Recorded agent batch"}}),
    );
    assert!(stopped.ok, "{stopped:?}");
    assert_eq!(stopped.undo_steps, 0);
    let recorded: Actions = serde_json::from_value(stopped.result.unwrap()["actions"].clone()).unwrap();
    assert_eq!(recorded.steps.len(), 2);
    assert_eq!(host.0.history_entries().last().unwrap().summary.verbs, ["move", "move", "set_paint"]);
    host.0.try_execute(varos_core::EditCommand::SelectPaths(vec![path])).unwrap();
    let before_depth = host.0.history_depths().0;
    recorded.replay(&mut host.0).unwrap();
    assert_eq!(host.0.history_depths().0, before_depth + 1);
}

#[test]
fn help_is_typed_opt_in_and_offline_hosts_do_not_open_os_surfaces() {
    let mut host = Fake(Editor::new());
    let mut service = Service::new("test".into());
    for action in ["docs", "shortcuts", "report_problem"] {
        let args = json!({"api":"1.2","action":action});
        assert!(varos_bridge::mcp::decode_tool("help", args.clone()).is_ok());
        assert!(
            varos_bridge::cli::decode(&serde_json::to_vec(&json!({"tool":"help","arguments":args})).unwrap()).is_ok()
        );
        let reply = call(&mut service, &mut host, "agent", "help", args);
        assert_eq!(reply.error.unwrap().code, "unsupported");
    }
    assert!(varos_bridge::mcp::decode_tool("help", json!({"api":"1.2","action":"shell","command":"rm"})).is_err());
    assert!(varos_bridge::mcp::schema("help", None).is_ok());
}

#[test]
fn history_list_matches_frozen_v12_result() {
    let mut host = Fake(Editor::new());
    host.0.try_execute(varos_core::EditCommand::SetBoardName("Poster".into())).unwrap();
    host.0.annotate_history(
        0,
        Actor::Agent { profile_id: "agent-a".into(), label: "Claude".into() },
        "Rename board".into(),
    );
    host.0.annotate_history_verbs(0, vec!["set_board_name".into()]);
    let mut service = Service::new("test".into());
    let reply = call(&mut service, &mut host, "agent-a", "history_list", json!({"api":"1.2","board":"b1"}));
    assert!(reply.ok);
    let mut result = reply.result.unwrap();
    assert!(result["entries"][0]["at"].as_u64().unwrap() > 0);
    result["entries"][0]["at"] = json!(0); // Normalize only the wall clock.
    let expected: Value = serde_json::from_slice(include_bytes!("phase9_fixtures/history-list-v12.json")).unwrap();
    assert_eq!(result, expected);
}
