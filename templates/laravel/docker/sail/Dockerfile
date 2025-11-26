# FROM ghcr.io/my-org/sail-php8.4:latest
FROM ghcr.io/mattbr1ght/sail-php8.4:latest
WORKDIR /var/www/html

COPY . .

RUN rm /bin/sh && ln -s /bin/bash /bin/sh
# RUN /bin/bash -c "source .env" && composer install --no-dev --optimize-autoloader
RUN composer install --no-dev --optimize-autoloader
RUN npm install && npm run build
COPY .env.coolify .env

EXPOSE 80
ENTRYPOINT ["start-container"]
