use mcpipe::backend::graphql::GraphQlBackend;
use mcpipe::backend::Backend;

#[tokio::test]
async fn discover_from_introspection_fixture() {
    let fixture = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/introspection.json")
    ).unwrap();
    let introspection: serde_json::Value = serde_json::from_str(&fixture).unwrap();

    let backend = GraphQlBackend::from_introspection(
        "http://localhost:4000/graphql".to_string(),
        introspection,
        vec![],
    );

    let cmds = backend.discover().await.unwrap();
    assert_eq!(cmds.len(), 2, "expected pets + createPet");

    let pets = cmds.iter().find(|c| c.name == "pets").expect("pets query");
    assert_eq!(pets.params.len(), 1);
    assert_eq!(pets.params[0].name, "limit");
    assert!(!pets.params[0].required);

    let create = cmds.iter().find(|c| c.name == "create-pet").expect("createPet mutation");
    let name_p = create.params.iter().find(|p| p.name == "name").expect("name param");
    assert!(name_p.required);
}
