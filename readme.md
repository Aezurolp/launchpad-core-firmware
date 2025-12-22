# Launchpad Injection CFW

Injection-based custom firmware for Novation Launchpad devices.

This project uses binary patching/injection techniques based on reverse engineering research to extend and modify stock firmware behavior.

> Note: This repository does **not** distribute Novation firmware.  
> You must provide your own official `.syx` update file. Use at your own risk.

## How does it work?

The official firmware updates are provided as `.syx` files, which are converted into raw `.bin` images.

The extracted stock firmware image is treated as the **base blob** (the “runtime”): it stays largely intact and continues to provide the original drivers, device logic, and internal services.

Custom code is then compiled and **injected into the existing firmware image**. Selected functions inside the stock firmware are **overridden** by patching call sites and/or function entry points to redirect execution into the injected implementations (hooks/detours).

The injected code can still call back into the original firmware functions, effectively using the stock firmware as a **driver layer** while replacing only the targeted behavior.

## Device Support

| Launchpad Device / OFW-Version | LEDs | Buttons | MIDI | Flash |
| ------------------------------ |:----:|:-------:|:----:|:-----:|
| Launchpad Pro Mk3 (385)        | ✅   | ✅      | ✅   | 🚧    |
| Launchpad Pro Mk1 (OSS)        | ✅   | ✅      | ✅   | ✅    |
| Launchpad Pro X (351)          | ✅   | ✅      | ✅   | 🚧    |
| Launchpad Mini Mk3 (407)       | ✅   | ✅      | ✅   | 🚧    |
| Launchpad Mk2 (171)            | 🚧   | 🚧      | 🚧   | 🚧    |

**Legend**
- ✅ Done and working
- 🚧 Work in progress
- ❌ Abandoned / no hope left

## Open note to Novation (a love letter, kind of)

I love the Launchpad platform. The Lightshow community has used and supported Launchpads for years, and many of us bought newer devices expecting the same reliability and workflow.

In the specific workflows we rely on for performances, the Launchpad Pro Mk3 has been a frustrating experience. This project exists because we needed practical fixes and community-driven improvements, while still keeping the stock firmware as the underlying driver layer.

I am genuinely open to collaboration. If Novation is interested, I’m happy to share findings, repro cases, and proposals that could help improve the official firmware for performance and lightshow use-cases.

**Contact:** contact@anthonyhfm.dev

## Credits

- [Kaskobi](https://youtube.com/@kaskobi) for creating the individual boot animations for all launchpads

The creation of this project was inspired by:

- [Gabriel Valky (gabonator)](https://github.com/gabonator)
- [Mat1jaczyyy](https://github.com/mat1jaczyyy)