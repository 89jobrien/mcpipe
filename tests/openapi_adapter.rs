use mcpipe::backend::openapi::OpenApiBackend;
use mcpipe::backend::Backend;
use mcpipe::domain::ParamLocation;

#[tokio::test]
async fn discover_from_file() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/petstore.json");
    let backend = OpenApiBackend::from_file(path).unwrap();
    let cmds = backend.discover().await.unwrap();

    assert_eq!(cmds.len(), 3);

    let list = cmds.iter().find(|c| c.name == "list-pets").expect("list-pets");
    assert_eq!(list.source_name, "listPets");
    assert_eq!(list.params.len(), 1);
    assert_eq!(list.params[0].name, "limit");
    assert!(!list.params[0].required);
    assert!(matches!(list.params[0].location, ParamLocation::Query));

    let create = cmds.iter().find(|c| c.name == "create-pet").expect("create-pet");
    let name_param = create.params.iter().find(|p| p.name == "name").expect("name param");
    assert!(name_param.required);
    assert!(matches!(name_param.location, ParamLocation::Body));

    let show = cmds.iter().find(|c| c.name == "show-pet-by-id").expect("show-pet-by-id");
    let id_param = show.params.iter().find(|p| p.name == "pet-id").expect("petId param");
    assert!(id_param.required);
    assert!(matches!(id_param.location, ParamLocation::Path));
}
