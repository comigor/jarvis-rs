FROM gcr.io/distroless/static

EXPOSE 8080

ENTRYPOINT ["/bin/jarvis"]

COPY jarvis /bin/jarvis
