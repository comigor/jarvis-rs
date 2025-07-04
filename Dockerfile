FROM gcr.io/distroless/static

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/jarvis"]

COPY jarvis /usr/local/bin/jarvis
