/*
 * Newer MSYS2 mingw-w64 GCC builds libstdc++.a's wide-char ctype/codecvt
 * facets (ctype_members.o, codecvt_members.o) against UCRT headers that
 * declare wctob/btowc as dllimport, generating references to the
 * function-pointer symbols __imp_wctob/__imp_btowc. The mingw-w64-crt
 * import libraries (libucrt*.a) shipped alongside this GCC don't provide
 * those two stubs (other UCRT wide-char functions like wctype/wcrtomb/
 * mbrtowc/iswctype are present), so provide them here pointing at
 * libmingwex's regular (non-dllimport) implementations.
 */
#include <wchar.h>

int (__cdecl *__imp_wctob)(wint_t) = wctob;
wint_t (__cdecl *__imp_btowc)(int) = btowc;
