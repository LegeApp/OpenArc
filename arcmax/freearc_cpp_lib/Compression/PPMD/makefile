include ../../common.mak

ALL: $(TEMPDIR)/C_PPMD.o

CODE_FLAGS  = -fno-exceptions -fno-rtti -Wall \
              -Wno-unknown-pragmas -Wno-sign-compare -Wno-conversion
OPT_FLAGS   = -O1 \
              -fomit-frame-pointer -fstrict-aliasing \
              -fforce-addr -funroll-all-loops
DEBUG_FLAGS = -g0
CFLAGS = $(CODE_FLAGS) $(OPT_FLAGS) $(DEBUG_FLAGS) $(DEFINES)

$(TEMPDIR)/C_PPMD.o: C_PPMD.cpp Model.cpp Model.hpp Coder.cpp Coder.hpp SubAlloc.hpp
	$(GCC) -c $(CFLAGS) -fexceptions -o $@ $<
