# Security policy

## Supported versions

Gecko is pre-1.0 and moves fast. Fixes land on `main` and go out in the next
release. Older versions are not patched.

| Version | Supported |
| ------- | --------- |
| 0.0.8   | Yes       |
| < 0.0.8 | No        |

## Reporting a vulnerability

Report privately through GitHub, using
[Report a vulnerability](https://github.com/punctuations/gecko/security/advisories/new)
on the repository's Security tab. That opens a private advisory only the
maintainers can see. Do not open a public issue for a security problem.

Include what you need to make the problem reproducible: the version or commit,
the platform, a program or input that triggers it, and what you expected to
happen instead.

Expect an acknowledgement within a week. If a report is confirmed, the advisory
tracks the fix and you will be credited in it unless you ask otherwise.

## Scope

The sandbox and the embedding limits are the parts with a real security
boundary, so problems in them matter most:

- Sandboxed code escaping its isolate, reaching the host, or reading another
  isolate's memory.
- Sandboxed code evading the step, wall-clock, or heap limits, so a runaway
  program cannot be stopped.
- Memory-safety faults in the C runtime that a Python program can reach: a
  crash, a read or write out of bounds, or a use after free.
- A frozen binary or a wheel install writing outside the paths it is given.

Gecko runs the code it is handed. Running an untrusted program outside the
sandbox is not a vulnerability, and neither is a program using unbounded memory
or time when no limits were set.

Bugs with no security consequence, meaning a wrong result, a missing feature, or
a divergence from CPython, belong in the public issue tracker instead.
