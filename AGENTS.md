# Agent rules

## No GitHub CI

Never add, configure, enable, or require CI through GitHub for this repository.
This includes GitHub Actions workflows and CI integrations triggered through
GitHub. Do not create `.github/workflows` or require GitHub CI status checks.

Run builds, tests, and performance measurements locally or on a machine the
user explicitly selects. Report the checks actually performed. Local
verification scripts are allowed; do not turn them into GitHub CI.
