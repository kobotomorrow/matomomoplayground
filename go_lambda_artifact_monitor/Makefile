BUILD_DIR := build
BINARY := bootstrap
ZIP_FILE := function.zip

.PHONY: build package clean

build:
	mkdir -p $(BUILD_DIR)
	GOOS=linux GOARCH=amd64 go build -o $(BUILD_DIR)/$(BINARY) .

package: build
	cd $(BUILD_DIR) && zip -q $(ZIP_FILE) $(BINARY)

clean:
	rm -rf $(BUILD_DIR)
