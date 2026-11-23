# Contributing

Thanks for wanting to improve DMZ SBMM Reader.

This project is **free and open source** under the **MIT License**. Anyone may use it, modify it, fork it, and redistribute it under the terms of that license.

## Ground rules

To keep this tool safe and trustworthy:

1. **Keep it read-only.** Do not add process injection, memory reading, hooking, DLL loading, TLS interception, or anti-cheat bypasses.
2. **Never modify game files.** Reports may only be written outside game folders.
3. **No personal data collection.** Do not upload, transmit, or phone home.
4. **No paid gating.** Do not add logins, subscriptions, ads, or paywalls.
5. **Be honest about limits.** This tool cannot prove server-side SBMM. Changes should not overstate what the data shows.

## Good contributions

- support for more COD install layouts
- clearer report formatting
- better string detection with fewer false positives
- cross-platform support where possible, while preserving read-only behavior
- documentation improvements
- tests and syntax checks

## Pull request checklist

- [ ] The change is read-only.
- [ ] No game files are written to.
- [ ] No network calls were added.
- [ ] No anti-cheat bypass or injection was added.
- [ ] README was updated if behavior changed.
- [ ] Scripts pass PowerShell syntax checks.

## License

By contributing, you agree that your contribution is licensed under the MIT License.
