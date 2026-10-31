use e2e::Services;

#[tokio::test]
#[ignore = "needs the built service binaries; run by the e2e CI job, see e2e/README.md"]
async fn all_services_answer_their_health_check() {
    let services = Services::start()
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    let client = reqwest::Client::new();
    for (name, base) in [
        ("registry", services.registry_url()),
        ("fly-mock", services.fly_mock_url()),
        ("family-app", services.family_app_url()),
    ] {
        let response = client
            .get(format!("{base}/healthz"))
            .send()
            .await
            .unwrap_or_else(|err| panic!("{name}: {err}"));
        assert!(
            response.status().is_success(),
            "{name}: {}",
            response.status()
        );
        assert_eq!(response.text().await.unwrap(), "ok", "{name}");
    }
}
