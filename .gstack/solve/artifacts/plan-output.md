# Phase 2: PLAN — Go-to-Market Execution Plan

## Architecture Decision
**Dual-track**: Quick monetization (sponsorship + consulting) while building the GitHub App as the first paid product.

---

## 30-Day Launch Plan

### Week 1: Foundation

| Day | Task | Output |
|---|---|---|
| 1 | Create falcon.dev landing page (GitHub Pages or Vercel) | Live URL |
| 2 | Write Getting Started docs (install, first analysis, CI setup) | docs/ |
| 3 | Create Dart wrapper package for pub.dev | pub.dev listing |
| 4 | Set up GitHub Sponsors ($5, $15, $50 tiers) | Sponsor button on repo |
| 5 | Record 2-minute demo video (terminal recording) | YouTube/Loom link |

### Week 2: Launch

| Day | Task | Output |
|---|---|---|
| 6 | Publish Blog 1: "The 5 Bugs Every AI Puts in Your Flutter Code" | Medium / dev.to |
| 7 | Post to r/FlutterDev, Flutter Discord, Twitter | Community awareness |
| 8 | Submit to Product Hunt | Launch day traffic |
| 9 | Publish Blog 2: "I Ran Falcon on 50 AI-Generated Apps" | SEO content |
| 10 | Reach out to 5 Flutter YouTubers for coverage | Influencer pipeline |

### Week 3: GitHub App MVP

| Day | Task | Output |
|---|---|---|
| 11-12 | Build GitHub App (Probot or custom) — auto-run Falcon on PR | Working app |
| 13 | Add Stripe billing via GitHub Marketplace or custom checkout | Payment flow |
| 14 | Beta test with 3-5 volunteer repos | Bug fixing |
| 15 | Launch GitHub App — free for OSS, $9/repo for private | Revenue! |

### Week 4: Partnerships + Enterprise

| Day | Task | Output |
|---|---|---|
| 16 | Send Cursor partnership email (template ready) | Outreach |
| 17 | Send Windsurf + Copilot partnership emails | Outreach |
| 18 | Publish Blog 3: "Why AI-Generated Flutter Apps Crash" | SEO |
| 19 | Create enterprise inquiry form on falcon.dev | Lead capture |
| 20 | Submit CFP to FlutterCon / Droidcon | Speaking pipeline |

---

## What Needs to Be Built (Code)

### 1. Landing Page (falcon.dev)
- **Framework**: Static HTML or Next.js on Vercel
- **Pages**: Home (hero + pricing), Docs, Blog, Enterprise contact
- **Key sections**: Live demo GIF, pricing table, testimonials placeholder, GitHub stars badge
- **Time**: 1-2 days

### 2. pub.dev Dart Wrapper
- **Structure**: Dart package that downloads and wraps the Falcon binary
- **Files**: `pubspec.yaml`, `bin/falcon.dart` (downloads correct binary for platform)
- **Command**: `dart pub global activate falcon`
- **Time**: 1 day

### 3. GitHub App
- **Stack**: Node.js + Probot framework (or Rust with octocrab)
- **Flow**: PR opened → clone repo → run `falcon ai-score --json` → post PR comment
- **Hosting**: Fly.io or Railway ($5/month)
- **Billing**: GitHub Marketplace or Stripe
- **Time**: 3-5 days

### 4. Documentation Site
- **Framework**: VitePress, Docusaurus, or mdBook (Rust)
- **Content**: Getting started, CLI reference (auto-generated from --help), Rule catalog, API docs, MCP setup
- **Time**: 2 days

---

## Pricing Validation

### How to validate before building everything:

1. **Smoke test**: Put pricing on landing page BEFORE building the GitHub App. See if anyone clicks "Buy."
2. **Waitlist**: "Join the Falcon Cloud waitlist" — collect emails to gauge demand.
3. **Enterprise ping**: Email 5 Flutter teams you know. "Would you pay $29/seat for automated code quality enforcement?"
4. **Open source signal**: GitHub stars velocity after launch tells you about organic demand.

### Competitive pricing justification:
- DCM charges $19-80/month and is slower + less featured
- Falcon at $9/repo is cheaper than DCM's cheapest tier
- Free CLI means zero risk trial — pay only when you want team features

---

## Risk Assessment

| Risk | Likelihood | Mitigation |
|---|---|---|
| No one pays (free is good enough) | Medium | Focus on team features they can't get from CLI — dashboards, trends, shared conventions |
| GitHub App reliability issues | Medium | Start with beta users, monitor closely, invest in error handling |
| DCM copies AI features | Low | 2-year head start + open source community = unreplicable |
| Google ships built-in AI linter | Low | Our Flutter-specific depth + MCP integration is unique |
| Pricing too high | Low | $9/repo is very cheap. Start lower ($5) if needed |
| Pricing too low | Medium | Enterprise deals should be $29+ per seat. Don't undercharge. |

---

## Success Metrics (First 90 Days)

| Metric | Target |
|---|---|
| GitHub stars | 500+ |
| pub.dev installs | 1,000+ |
| GitHub App installs | 50+ repos |
| Monthly recurring revenue | $500+ |
| Enterprise inquiries | 5+ |
| Partnership responses | 2+ |
| Blog post total views | 10,000+ |
| Discord/community members | 100+ |

---

## Decision Log
- Scope: EXPANSION with phased approach
- First product: GitHub App ($9/repo)
- Pricing model: Free CLI + Paid team features
- Launch strategy: Content-led (blog + Product Hunt + community)
