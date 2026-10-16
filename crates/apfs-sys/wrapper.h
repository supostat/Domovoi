#define _DARWIN_C_SOURCE 1

#include <stdio.h>
#include <sys/attr.h>
#include <sys/stat.h>
#include <sys/vnode.h>
#include <sys/clonefile.h>
#include <sys/snapshot.h>
#include <sys/resource.h>
#include <sys/mount.h>
#include <sys/fcntl.h>
#include <copyfile.h>
#include <libproc.h>
#include <xattr_flags.h>
