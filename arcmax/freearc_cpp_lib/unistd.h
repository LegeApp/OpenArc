/* Minimal unistd.h replacement for Windows */
#ifndef UNISTD_H
#define UNISTD_H

#include <io.h>
#include <process.h>

/* Map POSIX functions to Windows equivalents */
#define access _access
#define chdir _chdir
#define chmod _chmod
#define close _close
#define creat _creat
#define dup _dup
#define dup2 _dup2
#define eof _eof
#define execle _execle
#define execlp _execlp
#define execv _execv
#define execve _execve
#define execvp _execvp
#define fileno _fileno
#define fstat _fstat
#define getpid _getpid
#define isatty _isatty
#define lseek _lseek
#define open _open
#define read _read
#define rmdir _rmdir
#define sbrk _sbrk
#define stat _stat
#define sysconf _sysconf
#define unlink _unlink
#define write _write

/* Constants */
#define F_OK 0
#define X_OK 1
#define W_OK 2
#define R_OK 4

#define STDIN_FILENO 0
#define STDOUT_FILENO 1
#define STDERR_FILENO 2

#endif /* UNISTD_H */
