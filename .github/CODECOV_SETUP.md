# Codecov Setup Instructions

To enable code coverage reporting for this repository:

## 1. Sign up for Codecov
1. Go to [codecov.io](https://about.codecov.io/)
2. Sign in with your GitHub account
3. Grant access to this repository

## 2. Get the Upload Token
1. Navigate to your repository in Codecov
2. Go to Settings → General
3. Copy the Repository Upload Token

## 3. Add the Token to GitHub Secrets
1. Go to your GitHub repository
2. Navigate to Settings → Secrets and variables → Actions
3. Click "New repository secret"
4. Name: `CODECOV_TOKEN`
5. Value: (paste the token from Codecov)

## 4. Update the CI Workflow (Optional)
Once the token is configured, you can update `.github/workflows/ci.yml`:
- Change `fail_ci_if_error: false` to `fail_ci_if_error: true`
- Remove `continue-on-error: true`

## Current Configuration
The codecov integration is currently configured to:
- Not fail the CI build if coverage upload fails
- Run in informational mode (won't block PRs)
- Ignore test files and the binary entry point

This allows the CI to pass while Codecov is not fully configured.