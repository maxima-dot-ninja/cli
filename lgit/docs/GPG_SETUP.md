# GPG Key Setup Guide

This guide walks you through setting up GPG commit signing for use with lgit.

## Why Sign Commits?

Signed commits prove that you are the author of the code. GitHub and GitLab display a "Verified" badge next to signed commits, increasing trust in your contributions.

## Quick Setup

### 1. Check for Existing Keys

```bash
gpg --list-secret-keys --keyid-format LONG
```

If you see output with `sec` lines, you already have keys. Skip to step 3.

### 2. Generate a New GPG Key

```bash
gpg --full-generate-key
```

When prompted:

| Prompt | Recommended Value |
|--------|------------------|
| Key type | `RSA and RSA` (default) |
| Key size | `4096` |
| Expiration | `1y` (1 year) or `0` (never) |
| Real name | Your full name |
| Email | **Must match your Git email** |
| Passphrase | Strong, memorable password |

### 3. Find Your Key ID

```bash
gpg --list-secret-keys --keyid-format LONG
```

Output looks like:

```
sec   rsa4096/ABCD1234EFGH5678 2024-01-01 [SC]
      1234567890ABCDEF1234567890ABCDEF12345678
uid                 [ultimate] Your Name <you@example.com>
ssb   rsa4096/WXYZ9876STUV5432 2024-01-01 [E]
```

Your key ID is the part after `rsa4096/` on the `sec` line: `ABCD1234EFGH5678`

### 4. Configure Git

```bash
# Set your signing key
git config --global user.signingkey ABCD1234EFGH5678

# (Optional) Sign all commits by default
git config --global commit.gpgsign true
```

### 5. Export Your Public Key

```bash
gpg --armor --export ABCD1234EFGH5678
```

Copy the entire output, including:
```
-----BEGIN PGP PUBLIC KEY BLOCK-----
...
-----END PGP PUBLIC KEY BLOCK-----
```

### 6. Add to GitHub/GitLab

**GitHub:**
1. Go to Settings → SSH and GPG keys
2. Click "New GPG key"
3. Paste your public key
4. Click "Add GPG key"

**GitLab:**
1. Go to Preferences → GPG Keys
2. Paste your public key
3. Click "Add key"

## Troubleshooting

### "gpg failed to sign the data"

Add this to your `~/.bashrc` or `~/.zshrc`:

```bash
export GPG_TTY=$(tty)
```

Then restart your terminal or run `source ~/.bashrc`.

### GPG Agent Issues

Restart the GPG agent:

```bash
gpgconf --kill gpg-agent
gpg-agent --daemon
```

### Wrong Email

Your GPG key email must match your Git email:

```bash
# Check your Git email
git config user.email

# If different, either:
# 1. Change your Git email
git config --global user.email "your-gpg-email@example.com"

# 2. Or add the email to your GPG key
gpg --edit-key ABCD1234EFGH5678
# Then use: adduid
```

### Key Expired

Extend your key's expiration:

```bash
gpg --edit-key ABCD1234EFGH5678
# At the gpg> prompt:
expire
# Follow prompts to set new expiration
save
```

## macOS Specific

Install GPG Suite for better integration:

```bash
brew install --cask gpg-suite
```

Or use pinentry-mac:

```bash
brew install pinentry-mac
echo "pinentry-program $(which pinentry-mac)" >> ~/.gnupg/gpg-agent.conf
gpgconf --kill gpg-agent
```

## Verifying It Works

Create a test commit:

```bash
echo "test" > test.txt
git add test.txt
git commit -S -m "test signed commit"
```

Verify the signature:

```bash
git log --show-signature -1
```

You should see "Good signature from..." in the output.

## Using with lgit

Once set up, lgit will automatically detect your GPG keys and prompt you to select one when committing:

```
? Select signing option
❯ 🔐 Your Name <you@example.com> (ABCD1234EFGH5678)
  📝 Commit without signing
```

If you don't have any GPG keys set up, lgit will offer to create an unsigned commit and remind you to run `lgit --gpginfo` for setup instructions.
