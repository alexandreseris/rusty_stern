# Guide for testing

- install kubernetes server (docker for desktop can provide one or you can use minikube)
- `kubectl apply -f ./test/kubernetes1.yml` for standard logs
- `kubectl apply -f ./test/kubernetes2.yml` for longer name and random sleeps
- `kubectl apply -f ./test/kubernetes3.yml` for instant logs
- run the project
