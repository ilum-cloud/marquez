FROM eclipse-temurin:17 AS base
WORKDIR /usr/src/app
COPY gradle gradle
COPY gradle.properties gradle.properties
COPY gradlew gradlew
COPY settings.gradle settings.gradle

RUN chmod +x ./gradlew
RUN sed -i 's/\r$//' ./gradlew

FROM base AS build
WORKDIR /usr/src/app
COPY build.gradle build.gradle
COPY api ./api
COPY clients/java ./clients/java
RUN ./gradlew clean :api:shadowJar --no-daemon --refresh-dependencies

FROM eclipse-temurin:17-jre-ubi10-minimal
# Install required runtime tools on UBI minimal and clean up cache
RUN microdnf -y update && \
    microdnf -y install postgresql bash dos2unix && \
    microdnf -y clean all
WORKDIR /usr/src/app
COPY --from=build /usr/src/app/api/build/libs/marquez-*.jar /usr/src/app
COPY marquez.dev.yml marquez.dev.yml
COPY docker/entrypoint.sh entrypoint.sh
RUN dos2unix entrypoint.sh && \
    chmod +x entrypoint.sh

EXPOSE 5000 5001
ENTRYPOINT ["/usr/src/app/entrypoint.sh"]
