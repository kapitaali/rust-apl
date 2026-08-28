CC = gcc
CFLAGS = -Wall -Wextra -g
APL_DIR = .
INCLUDE = -I$(APL_DIR)/include

# Library paths
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Linux)
    LIB_EXT = so
    LIB_PATH = $(APL_DIR)/target/release/libapl.so
    RUN_ENV = LD_LIBRARY_PATH=$(APL_DIR)/target/release
endif
ifeq ($(UNAME_S),Darwin)
    LIB_EXT = dylib
    LIB_PATH = $(APL_DIR)/target/release/libapl.dylib
    RUN_ENV = DYLD_LIBRARY_PATH=$(APL_DIR)/target/release
endif

.PHONY: all clean test

all: test_libapl

# Build the Rust library first
$(LIB_PATH):
	cd $(APL_DIR) && cargo build --release

# Compile the C test program
test_libapl: $(APL_DIR)/tests/test_libapl.c $(LIB_PATH)
	$(CC) $(CFLAGS) $(INCLUDE) -o $@ $(APL_DIR)/tests/test_libapl.c -L$(APL_DIR)/target/release -lapl -lm -lpthread -ldl

# Run the test
test: test_libapl
	$(RUN_ENV) ./test_libapl

clean:
	rm -f test_libapl
