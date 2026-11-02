# family-ai app

The front end, in Flutter: one codebase for the web, iPhone and Android. So far it is a placeholder screen, built and tested for the web only.

The dev container has the pinned Flutter, Chrome and chromedriver, on amd64 only (see `.devcontainer/README.md`).

## Checks

Run these in `app/`. The `app` job in `.github/workflows/devcontainer-ci.yml` runs the same ones.

```sh
flutter analyze
flutter test
flutter build web
```

The integration test runs in Chrome and needs chromedriver running:

```sh
chromedriver --port=4444 &
flutter drive --driver=test_driver/integration_test.dart \
  --target=integration_test/smoke_test.dart -d web-server \
  --browser-name=chrome --web-browser-flag=--no-sandbox
kill %1
```

`--no-sandbox` is there because Chrome's sandbox needs privileges a container lacks. Only use it for this app's own pages.
