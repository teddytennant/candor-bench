# Security Policy

candor-bench is a research and evaluation tool. It runs language models against
a fixed dataset and scores the outputs — it does not run untrusted input as
code, store credentials, or expose a network service. API keys are read from
the environment and only used to call the provider you configured.

## Reporting a vulnerability

Email **teddytennant@icloud.com** with details and a repro. Please don't open a
public issue for anything exploitable. Expect an initial reply within a few days.

In scope: anything that could leak a user's API key, execute arbitrary code
from a dataset file or model response, or otherwise harm someone running the
suite.

## Not a vulnerability: the factuality dataset

The `factuality` eval **deliberately** contains textbook-level factual content
about sensitive history topics — death tolls for the Holocaust, Hiroshima,
Tiananmen, and similar widely-documented events. This is the point of the eval:
it measures whether post-trained models will give specific, correct answers to
public, well-defined questions instead of refusing or hedging. The dataset
excludes operational-uplift content by construction. Please do not file this as
a security or safety issue.
