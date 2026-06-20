SRC_DIR = src
BUILD_DIR = build
SRCM = main

BIN = $(BUILD_DIR)/$(SRCM).bin


ifneq ($(filter set,$(firstword $(MAKECMDGOALS))),)
  TARGET_NAME := $(word 2,$(MAKECMDGOALS))
  $(eval $(TARGET_NAME):;@:)
endif


see:
	echo $(SRCM)

set:
	

	sed -i 's/^SRCM =.*/SRCM = $(TARGET_NAME)/' Makefile
	@echo "Successfully changed SRCM to $(TARGET_NAME)!"

$(SRCM): $(SRC_DIR)/$(SRCM).asm
	nasm -f bin $(SRC_DIR)/$(SRCM).asm -o $(BIN)
	
	

run: $(BIN)
	#qemu-system-x86_64 -drive format=raw,file=$(BUILD_DIR)/$(SRCM).bin -display gtk
	qemu-system-x86_64 -drive format=raw,file=$(BIN) -display gtk

clean:
	rm -f $(BIN)