# Falcon GitHub App

Auto-analyze Flutter PRs with Falcon and post quality reports as comments.

## How It Works

1. Developer opens a PR on a Flutter repository
2. Falcon GitHub App triggers on `pull_request` events
3. Falcon analyzes the changed files
4. Results posted as a PR comment with:
   - AI Code Quality Score (0-100)
   - Issue breakdown by rule
   - Errors that must be fixed
   - Trend vs. previous analysis

## Setup

### Option 1: GitHub Actions (Free, self-hosted)

Add to your repo at `.github/workflows/falcon.yml`:

```yaml
name: Falcon Analysis
on:
  pull_request:
    types: [opened, synchronize]

jobs:
  falcon:
    runs-on: ubuntu-latest
    permissions:
      pull-requests: write
      contents: read
    steps:
      - uses: actions/checkout@v4

      - name: Install Falcon
        run: cargo install --git https://github.com/viveky259259/falcon

      - name: Run AI Score
        run: falcon ai-score . --json > falcon-score.json

      - name: Post PR Comment
        run: falcon pr-comment . --dry-run > comment.md

      - name: Comment on PR
        uses: marocchino/sticky-pull-request-comment@v2
        with:
          path: comment.md
```

### Option 2: Hosted GitHub App (Coming Soon)

Install the Falcon GitHub App from GitHub Marketplace:
- Free for public repositories
- $9/repo/month for private repositories
- Automatic analysis on every PR
- Team dashboard at falcon.dev

## Local Development

```bash
# Clone this repo
git clone https://github.com/viveky259259/falcon
cd falcon/github-app

# Install dependencies
npm install

# Set environment variables
cp .env.example .env
# Edit .env with your GitHub App credentials

# Run locally
npm start
```

## Environment Variables

| Variable | Description |
|---|---|
| `APP_ID` | GitHub App ID |
| `PRIVATE_KEY` | GitHub App private key (PEM) |
| `WEBHOOK_SECRET` | Webhook secret for verification |
| `STRIPE_KEY` | Stripe API key (for paid repos) |
