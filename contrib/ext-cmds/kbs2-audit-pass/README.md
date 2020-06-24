`kbs2-audit-pass`
=================

`kbs2-audit-pass` is an external `kbs2` command that uses the
Have I Been Pwned? ["Pwned Passwords"](https://haveibeenpwned.com/API/v3#PwnedPasswords)
service to check whether a user's passwords are included in a breach.

## Setup

`curl` and `shasum` are required.

## Usage

Auditing every login in the store:

```bash
kbs2 audit-pass --all
```

Auditing just the listed logins:

```bash
kbs2 audit-pass email facebook amazon
```
