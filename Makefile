DOCKER_USERNAME ?= lucasmolinari
API_IMAGE_NAME ?= $(DOCKER_USERNAME)/rbe26-api
LB_IMAGE_NAME ?= $(DOCKER_USERNAME)/rbe26-lb
VERSION ?= latest

.PHONY: all build-api build-lb build push-api push-lb push clean

all: push-api push-lb

build-api:
	@echo "Building API image: $(API_IMAGE_NAME):$(VERSION)"
	docker build \
		--target api \
		-t $(API_IMAGE_NAME):$(VERSION) \
		.

build-lb:
	@echo "Building Load Balancer image: $(LB_IMAGE_NAME):$(VERSION)"
	docker build \
		--target lb \
		-t $(LB_IMAGE_NAME):$(VERSION) \
		.

build: build-api build-lb

push-api: build-api
	@echo "Pushing API image: $(API_IMAGE_NAME):$(VERSION)"
	docker push $(API_IMAGE_NAME):$(VERSION)

push-lb: build-lb
	@echo "Pushing Load Balancer image: $(LB_IMAGE_NAME):$(VERSION)"
	docker push $(LB_IMAGE_NAME):$(VERSION)

push: push-api push-lb

clean:
	@echo "Removing locally built images..."
	docker rmi $(API_IMAGE_NAME):$(VERSION) 2>/dev/null || true
	docker rmi $(LB_IMAGE_NAME):$(VERSION) 2>/dev/null || true
