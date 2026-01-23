.PHONY: backend-dev backend-test backend-app-auth frontend-dev


BE_DIR := backend
FE_DIR := frontend
IMAGE_NAME := push-cache

status-ports:
	@lsof -i tcp:8080
	@lsof -i tcp:4200


backend-dev: export CAPTURE_LOG=INFO
backend-dev:
	cd ${BE_DIR} && cargo watch  -x "run -- start --config test-data/config-localhost.yaml --secrets test-data/secrets"

backend-test:
	cd ${BE_DIR} && cargo watch --ignore test_data -x "test"

backend-docker: PKG_NAME=push-cache
backend-docker:
	{ \
	docker buildx build ${BE_DIR} -t $(IMAGE_NAME)-backend -f ${BE_DIR}/Dockerfile; \
	docker image ls $(IMAGE_NAME)-backend; \
	}

backend-docker-run: backend-docker
	docker run -it --rm --name $(IMAGE_NAME)-backend -p 8080:8080 --mount type=bind,src=$(PWD)/${BE_DIR}/test-data,dst=/test-data  \
	-e CAPTURE_LOG=INFO \
	$(IMAGE_NAME)-backend start --config /test-data/config-localhost.yaml --secrets /test-data/secrets





frontend-dev:
	cd frontend && ng serve

frontend-docker: PKG_NAME=push-cache
frontend-docker:
	{ \
	docker build ${FE_DIR} -t $(IMAGE_NAME)-frontend -f ${FE_DIR}/Dockerfile --build-arg PKG_NAME=${PKG_NAME}; \
	docker image ls $(IMAGE_NAME)-frontend; \
	}

frontend-docker-run: frontend-docker
	docker run -it --rm -p 4201:8080 --name $(IMAGE_NAME)-frontend $(IMAGE_NAME)-frontend
