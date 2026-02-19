# Lambdaform Content Marketing Plan

## Phase 1: Launch (Week 1)

### HN Show HN
- See `show-hn-draft.md`
- Target: Tuesday or Wednesday, 9-10am ET

### Reddit Cross-Posts (day of or day after HN)
- r/aws — "Built a Terraform-native local Lambda emulator"
- r/terraform — "Lambdaform: local dev server that reads your .tf files"
- r/serverless — same angle as HN
- r/rust — "Built a Lambda emulator in Rust" (if HN goes well)

### Twitter/X (@conner__v)
- Launch thread (5-6 tweets): problem → solution → demo GIF → link
- Engage serverless/Terraform community accounts

## Phase 2: Content (Weeks 2-8, one post every 2 weeks)

### Article 1 (Week 2): "Terraform + Lambda Local Dev in 2026: Your Options"
- Platform: dev.to
- Angle: Neutral comparison (LocalStack vs SAM vs serverless-offline vs Lambdaform)
- Link to Lambdaform as one option, not a sales pitch

### Article 2 (Week 4): "Building a Lambda Emulator in Rust: Parsing HCL Without the HCL Library"
- Platform: dev.to + r/rust
- Angle: Technical deep-dive, interesting Rust patterns
- Appeals to Rust community (potential contributors)

### Article 3 (Week 6): "From Zero to Serverless Locally: A Step-by-Step Terraform + Lambdaform Tutorial"
- Platform: dev.to
- Angle: Practical tutorial, beginner-friendly
- SEO play for "terraform lambda local development"

### Article 4 (Week 8): "Dogfooding Your Dev Tools: What I Learned Building a Real App with Lambdaform"
- Platform: dev.to
- Angle: Civic Scanner story, bugs found, lessons learned
- Authenticity play

## Phase 3: Community (Ongoing)

### GitHub Discussions
- Enable: General, Q&A, Ideas, Show and Tell
- Seed with 1-2 discussion topics (roadmap feedback, runtime requests)

### Good First Issues
- Label 2-3 issues for new contributors:
  - "Add --port flag to override default port" (if not done)
  - "Support for additional Lambda environment variables"
  - Documentation improvements

### OpenTofu Community
- Post in OpenTofu Discord/forums about compatibility
- Offer to add "Works with OpenTofu" badge if they have a program

## Metrics to Track
- GitHub stars (vanity but social proof)
- npm downloads (npx lambdaform)
- Homebrew installs
- GitHub issues/PRs from non-Conner users
- dev.to article views/reactions
