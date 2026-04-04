#[cfg(feature = "integration")]
mod tests {
    #[tokio::test]
    async fn mcpipe_cli_list_doob() {
        use mcpipe::backend::cli::CliBackend;
        use mcpipe::backend::Backend;
        let backend = CliBackend::new("doob");
        let cmds = backend.discover().await.unwrap();
        assert!(cmds.iter().any(|c| c.name == "todo-list"));
    }

    #[tokio::test]
    async fn gen_openapi_from_doob() {
        use mcpipe::backend::cli::CliBackend;
        use mcpipe::backend::Backend;
        use mcpipe::openapi_gen;
        let backend = CliBackend::new("doob");
        let cmds = backend.discover().await.unwrap();
        let doc = openapi_gen::generate("doob", "0.1.0", &cmds);
        let yaml = openapi_gen::to_yaml(&doc).unwrap();
        assert!(yaml.contains("openapi: 3.1.0"));
        assert!(yaml.contains("/todo-list"));
    }
}
