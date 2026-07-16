// CBA test framework macros for ABE test files.
// These are provided by the CBA test runner at runtime, but we also define them
// here so HEMTT's SQF parser can statically validate the test files.
// If CBA redefines these at runtime, the redefinition is silent (identical content)
// or logged as a warning (different content) — either way the test works.

#ifndef TEST_DEFINED
#define TEST_DEFINED(config, name) \
    if (isNull (config)) then { \
        diag_log text format ["FAILED: %1 — config entry not found", name]; \
    } else { \
        diag_log text format ["PASSED: %1", name]; \
    }
#endif

#ifndef TEST_OP
#define TEST_OP(a, op, b, name) \
    if (a op b) then { \
        diag_log text format ["PASSED: %1 (value: %2)", name, a]; \
    } else { \
        diag_log text format ["FAILED: %1 (value: %2, expected: %3)", name, a, b]; \
    }
#endif

#ifndef TEST_LOGIC
#define TEST_LOGIC(condition, name) \
    if (condition) then { \
        diag_log text format ["PASSED: %1", name]; \
    } else { \
        diag_log text format ["FAILED: %1", name]; \
    }
#endif
