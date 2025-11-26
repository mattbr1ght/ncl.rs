FROM node:latest
WORKDIR /usr/src/app

COPY . .
RUN npm install

ENV PORT=80
ENV APP_PORT=80
EXPOSE 80

ENTRYPOINT [ "npm", "start", "--", "--port", "80" ] 
