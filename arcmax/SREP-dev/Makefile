# SREP Library Makefile
CXX = g++
AR = ar
CXXFLAGS = -O3 -std=c++11 -Wall -Wextra
LDFLAGS = -pthread

# Output directories
LIBDIR = lib
BINDIR = bin
OBJDIR = obj

# Library name
LIBNAME = libsrep.a

# Source files
LIB_SRCS = srep_lib.cpp
LIB_OBJS = $(OBJDIR)/srep_lib.o

# Example source
EXAMPLE_SRC = example.cpp
EXAMPLE_BIN = $(BINDIR)/srep_example

# Targets
.PHONY: all clean lib example dirs

all: dirs lib example

dirs:
	@mkdir -p $(OBJDIR) $(LIBDIR) $(BINDIR)

# Build library
lib: dirs $(LIBDIR)/$(LIBNAME)

$(LIBDIR)/$(LIBNAME): $(LIB_OBJS)
	$(AR) rcs $@ $^
	@echo "Library built: $@"

$(OBJDIR)/srep_lib.o: srep_lib.cpp srep_lib.h
	$(CXX) $(CXXFLAGS) -c $< -o $@

# Build example
example: dirs lib $(EXAMPLE_BIN)

$(EXAMPLE_BIN): $(EXAMPLE_SRC) $(LIBDIR)/$(LIBNAME)
	$(CXX) $(CXXFLAGS) $< -L$(LIBDIR) -lsrep $(LDFLAGS) -o $@
	@echo "Example built: $@"

# Build with FreeArc dependencies
with-deps: CXXFLAGS += -DHAVE_FREEARC_DEPS -I../
with-deps: all

# Clean build artifacts
clean:
	rm -rf $(OBJDIR) $(LIBDIR) $(BINDIR)
	@echo "Cleaned build artifacts"

# Install library (requires sudo)
install: lib
	install -m 644 srep_lib.h /usr/local/include/
	install -m 644 $(LIBDIR)/$(LIBNAME) /usr/local/lib/
	@echo "Installed to /usr/local"

# Uninstall
uninstall:
	rm -f /usr/local/include/srep_lib.h
	rm -f /usr/local/lib/$(LIBNAME)
	@echo "Uninstalled from /usr/local"

# Build with debug symbols
debug: CXXFLAGS = -g -O0 -std=c++11 -Wall -Wextra -DDEBUG
debug: all

# Build with address sanitizer
asan: CXXFLAGS += -fsanitize=address -fno-omit-frame-pointer
asan: LDFLAGS += -fsanitize=address
asan: all

# Build with thread sanitizer
tsan: CXXFLAGS += -fsanitize=thread -fno-omit-frame-pointer
tsan: LDFLAGS += -fsanitize=thread
tsan: all

# Static analysis
analyze:
	clang-tidy srep_lib.cpp -- $(CXXFLAGS)

# Format code
format:
	clang-format -i srep_lib.cpp srep_lib.h example.cpp

# Generate documentation
docs:
	doxygen Doxyfile 2>/dev/null || echo "Doxygen not installed"

help:
	@echo "SREP Library Makefile"
	@echo ""
	@echo "Targets:"
	@echo "  all        - Build library and example (default)"
	@echo "  lib        - Build library only"
	@echo "  example    - Build example program"
	@echo "  with-deps  - Build with FreeArc dependencies"
	@echo "  debug      - Build with debug symbols"
	@echo "  asan       - Build with address sanitizer"
	@echo "  tsan       - Build with thread sanitizer"
	@echo "  install    - Install library to /usr/local"
	@echo "  uninstall  - Remove from /usr/local"
	@echo "  clean      - Remove build artifacts"
	@echo "  analyze    - Run clang-tidy"
	@echo "  format     - Format code with clang-format"
	@echo "  docs       - Generate documentation"
	@echo "  help       - Show this help"
