FROM node:latest
WORKDIR /usr/src/app

COPY . .
RUN npm install

ENV PORT=80
EXPOSE 80

CMD [ "npm", "start" ] 
