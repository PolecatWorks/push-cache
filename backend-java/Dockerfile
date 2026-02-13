# Stage 1: Build the application
FROM gradle:8.8-jdk21 AS build
WORKDIR /app
COPY --chown=gradle:gradle . .

RUN --mount=type=secret,id=ca.crt,target=/usr/local/share/ca-certificates/ca.crt \
    update-ca-certificates && \
    keytool -list -keystore $JAVA_HOME/lib/security/cacerts -storepass changeit

RUN --mount=type=cache,target=/root/.m2 gradle bootJar --no-daemon

# Stage 2: Create a custom JRE
FROM eclipse-temurin:21 AS jre-build
WORKDIR /app

# Create a custom Java runtime
RUN $JAVA_HOME/bin/jlink \
         --add-modules java.base,java.logging,java.naming,java.desktop,java.management,java.security.jgss,java.instrument,java.sql,jdk.unsupported,java.net.http,java.scripting,java.rmi,java.xml,jdk.crypto.ec \
         --strip-debug \
         --no-man-pages \
         --no-header-files \
         --compress=2 \
         --output /javaruntime

# Stage 3: Create the final image
FROM ubuntu:24.04
ENV JAVA_HOME=/opt/java/openjdk
ENV PATH="${JAVA_HOME}/bin:${PATH}"

# Copy the custom JRE
COPY --from=jre-build /javaruntime $JAVA_HOME

WORKDIR /app
# Copy the built application
COPY --from=build /app/build/libs/*.jar app.jar

EXPOSE 8080
ENTRYPOINT ["java", "-jar", "app.jar"]
