# Push Cache

* [![backend Docker](https://github.com/PolecatWorks/push-cache/actions/workflows/backend-docker-publish.yml/badge.svg)](https://github.com/PolecatWorks/push-cache/actions/workflows/backend-docker-publish.yml)
* [![frontend Docker](https://github.com/PolecatWorks/push-cache/actions/workflows/frontend-docker-publish.yml/badge.svg)](https://github.com/PolecatWorks/push-cache/actions/workflows/frontend-docker-publish.yml)
* [![Helm](https://github.com/PolecatWorks/push-cache/actions/workflows/helm-publish.yaml/badge.svg)](https://github.com/PolecatWorks/push-cache/actions/workflows/helm-publish.yaml)


This app provides a high-performance caching and message relay system using Kafka.


The backend will be written in Rust.

The frontend will be written in Angular with Material Design.

The app will be containerized and deployed to a Kubernetes cluster.

The app will be deployed to a Kubernetes cluster.

## Tech Stack

### Backend
- Rust
- Axum
- Kubernetes

### Frontend
- Angular
- Material Design
- Kubernetes

# Getting Started

If you have Make installed, you can use the following commands to get started:

    make backend-dev
    make frontend-dev

Or with Docker:
    make backend-docker-run
    make frontend-docker-run
### Authentication

Get the relevant APIs:

    curl http://keycloak.k8s/auth/realms/dev/.well-known/openid-configuration

To obtain a JWT for user `jon snow` in the `dev` realm:

    curl -X POST http://keycloak.k8s/auth/realms/dev/protocol/openid-connect/token \
    -H "Content-Type: application/x-www-form-urlencoded" \
    -d "grant_type=password" \
    -d "username=johnsnow" \
    -d "password=johnsnow" \
    -d "client_id=app-ui"




Using python

    ```python
    import aiohttp

    async def get_jwt_token():
        url = "http://keycloak.k8s/auth/realms/dev/protocol/openid-connect/token"
        data = {
            "grant_type": "password",
            "username": "johnsnow",
            "password": "johnsnow",
            "client_id": "app-ui"
        }
        async with aiohttp.ClientSession() as session:
            async with session.post(url, data=data) as response:
                return await response.json()
    ```


# Testing

install the test environment to run the tests

    python3 -m venv venv
    source venv/bin/activate
    pip install poetry
    poetry install --with dev
