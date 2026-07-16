#define MAINPREFIX z
#define PREFIX abe

#include "script_version.hpp"

#define VERSION MAJOR.MINOR.PATCHLVL.BUILD
#define VERSION_AR MAJOR,MINOR,PATCHLVL,BUILD

#define ABE_TAG ABE

// MINIMAL required version for the Mod. When not defined all versions are valid.
#define REQUIRED_VERSION 2.00

#ifdef COMPONENT_BEAUTIFIED
    #define COMPONENT_NAME QUOTED(Addon - COMPONENT_BEAUTIFIED)
#else
    #define COMPONENT_NAME QUOTED(Addon - COMPONENT)
#endif
