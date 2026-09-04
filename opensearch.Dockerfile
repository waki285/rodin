FROM opensearchproject/opensearch:3.8.0

# Install Japanese analyzers once at build time to avoid re-downloading on each up
RUN bin/opensearch-plugin install --batch analysis-kuromoji \
    && bin/opensearch-plugin install --batch analysis-icu

# Use upstream entrypoint
ENTRYPOINT ["/usr/share/opensearch/opensearch-docker-entrypoint.sh"]

