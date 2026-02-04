FROM gradle:8.8-jdk21 AS build
WORKDIR /app
USER root
RUN chown -R gradle:gradle /app
USER gradle
COPY --chown=gradle:gradle . .
RUN gradle bootJar --no-daemon

FROM eclipse-temurin:21
WORKDIR /app
COPY --from=build /app/build/libs/*.jar app.jar
EXPOSE 8080
ENTRYPOINT ["java", "-jar", "app.jar"]
