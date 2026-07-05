BUILD_DIR := build
BINARY := bootstrap
ZIP_FILE := function.zip
BUILD_IMAGE := public.ecr.aws/amazonlinux/amazonlinux:2023

.PHONY: build build-local package package-local clean

build-local:
	mkdir -p $(BUILD_DIR)
	CGO_ENABLED=1 GOOS=linux GOARCH=amd64 go build -o $(BUILD_DIR)/$(BINARY) .

build:
	docker run --rm \
		-v $(CURDIR):/workspace \
		-v go-build-curated-running-distance-mod:/go/pkg/mod \
		-v go-build-curated-running-distance-cache:/root/.cache/go-build \
		-w /workspace \
		$(BUILD_IMAGE) \
		bash -c 'dnf install -y golang gcc gcc-c++ && mkdir -p $(BUILD_DIR) && CGO_ENABLED=1 GOOS=linux GOARCH=amd64 go build -o $(BUILD_DIR)/$(BINARY) . && chmod -R a+rwX $(BUILD_DIR)'

package: build
	cd $(BUILD_DIR) && zip -q $(ZIP_FILE) $(BINARY)

package-local: build-local
	cd $(BUILD_DIR) && zip -q $(ZIP_FILE) $(BINARY)

clean:
	rm -rf $(BUILD_DIR)
