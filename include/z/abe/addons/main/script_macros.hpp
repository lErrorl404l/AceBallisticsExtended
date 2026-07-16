// CBA-style macros for ABE

#define QUOTE(var) #var
#define DOUBLES(var1,var2) var1##_##var2
#define TRIPLES(var1,var2,var3) var1##_##var2##_##var3

#define ADDON DOUBLES(PREFIX,COMPONENT)

#ifdef RECOMPILE
    #define RECOMPILE_RECOMENDATIONS 1
#else
    #define RECOMPILE_RECOMENDATIONS 0
#endif

#define FUNC(var) TRIPLES(ADDON,fnc,var)
#define GVAR(var) ADDON##_##var
