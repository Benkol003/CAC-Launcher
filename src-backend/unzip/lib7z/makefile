7ZPATH = ../../..

!include "$(7ZPATH)/CPP/Build.mak"
!include "$(7ZPATH)/CPP/7zip/Crc.mak"
!include "$(7ZPATH)/CPP/7zip/LzmaDec.mak"

CFLAGS = $(CFLAGS) /WX- -DZ7_EXTRACT_ONLY

PROG = lib.exe

C_OBJS = \
  $O\7zAlloc.obj \
  $O\7zBuf.obj \
  $O\7zFile.obj \
  $O\7zDec.obj \
  $O\7zArcIn.obj \
  $O\7zStream.obj \
  $O\Bcj2.obj \
  $O\Bra.obj \
  $O\Bra86.obj \
  $O\BraIA64.obj \
  $O\CpuArch.obj \
  $O\Delta.obj \
  $O\Lzma2Dec.obj \
  $O\LzmaDec.obj \

7Z_OBJS = \
  $O\lib.obj

OBJS = \
  $O\Precomp.obj \
  $(7Z_OBJS) \
  $(C_OBJS) \
  $(ASM_OBJS) \

$(7Z_OBJS): ./src/$(*B).c
	$(CCOMPL_USE)
$(C_OBJS): $(7ZPATH)/C/$(*B).c 
  $(CCOMPL_USE)

$O\Precomp.obj: ./src/precomp.c
	$(CCOMPL_PCH)

!include "$(7ZPATH)/C/Asm_c.mak"
