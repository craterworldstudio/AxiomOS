CC = gcc
CFLAGS = -m64 -ffreestanding -fno-pie -fno-stack-protector -nostdlib -mno-red-zone -Iinclude -O2
LDFLAGS = -m elf_x86_64 -T linker.ld
NASM = nasm

OUT_DIR = ../target/userland_bin
BUILD_DIR = build

ALL_BINS = $(OUT_DIR)/echo.bin $(OUT_DIR)/whoami.bin $(OUT_DIR)/ls.bin $(OUT_DIR)/cat.bin $(OUT_DIR)/axe.bin

all: $(BUILD_DIR) $(OUT_DIR) $(ALL_BINS)

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(OUT_DIR):
	mkdir -p $(OUT_DIR)

$(BUILD_DIR)/crt0.o: lib/crt0.s
	$(CC) $(CFLAGS) -c lib/crt0.s -o $(BUILD_DIR)/crt0.o

$(BUILD_DIR)/libaxiom.o: lib/libaxiom.c include/axiom.h
	$(CC) $(CFLAGS) -c lib/libaxiom.c -o $(BUILD_DIR)/libaxiom.o

$(OUT_DIR)/echo.bin: $(BUILD_DIR)/crt0.o $(BUILD_DIR)/libaxiom.o bin/echo.c
	$(CC) $(CFLAGS) -c bin/echo.c -o $(BUILD_DIR)/echo.o
	$(LD) $(LDFLAGS) -o $(BUILD_DIR)/echo.elf $(BUILD_DIR)/crt0.o $(BUILD_DIR)/echo.o $(BUILD_DIR)/libaxiom.o
	objcopy -O binary $(BUILD_DIR)/echo.elf $(OUT_DIR)/echo.bin

$(OUT_DIR)/whoami.bin: bin/whoami.asm
	$(NASM) -f bin bin/whoami.asm -o $(OUT_DIR)/whoami.bin

$(OUT_DIR)/ls.bin: $(BUILD_DIR)/crt0.o $(BUILD_DIR)/libaxiom.o bin/ls.c
	$(CC) $(CFLAGS) -c bin/ls.c -o $(BUILD_DIR)/ls.o
	$(LD) $(LDFLAGS) -o $(BUILD_DIR)/ls.elf $(BUILD_DIR)/crt0.o $(BUILD_DIR)/ls.o $(BUILD_DIR)/libaxiom.o
	objcopy -O binary $(BUILD_DIR)/ls.elf $(OUT_DIR)/ls.bin

$(OUT_DIR)/cat.bin: $(BUILD_DIR)/crt0.o $(BUILD_DIR)/libaxiom.o bin/cat.c
	$(CC) $(CFLAGS) -c bin/cat.c -o $(BUILD_DIR)/cat.o
	$(LD) $(LDFLAGS) -o $(BUILD_DIR)/cat.elf $(BUILD_DIR)/crt0.o $(BUILD_DIR)/cat.o $(BUILD_DIR)/libaxiom.o
	objcopy -O binary $(BUILD_DIR)/cat.elf $(OUT_DIR)/cat.bin

$(OUT_DIR)/axe.bin: $(BUILD_DIR)/crt0.o $(BUILD_DIR)/libaxiom.o bin/axe.c
	$(CC) $(CFLAGS) -c bin/axe.c -o $(BUILD_DIR)/axe.o
	$(LD) $(LDFLAGS) -o $(BUILD_DIR)/axe.elf $(BUILD_DIR)/crt0.o $(BUILD_DIR)/axe.o $(BUILD_DIR)/libaxiom.o
	objcopy -O binary $(BUILD_DIR)/axe.elf $(OUT_DIR)/axe.bin

clean:
	rm -rf $(BUILD_DIR) $(OUT_DIR)
