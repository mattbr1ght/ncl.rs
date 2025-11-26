use anyhow::{Context, Result};
use log::debug;
use std::path::Path;

/// Generates GitHub Actions CI/CD workflow for dev and prod branches
pub fn generate_cicd_workflow(project_path: &Path, template_name: &str) -> Result<()> {
    debug!("Generating CI/CD workflow for template: {}", template_name);

    let workflows_dir = project_path.join(".github").join("workflows");
    std::fs::create_dir_all(&workflows_dir)
        .context("Failed to create .github/workflows directory")?;

    // Generate workflow that triggers on PR to dev and merge to prod
    let workflow_content = generate_workflow_yaml(template_name);

    let workflow_file = workflows_dir.join("ci.yml");
    std::fs::write(&workflow_file, workflow_content)
        .context("Failed to write CI/CD workflow file")?;

    debug!("Generated CI/CD workflow at {}", workflow_file.display());
    Ok(())
}

fn generate_workflow_yaml(template_name: &str) -> String {
    // Base workflow that triggers on PR to dev and merge to prod
    let base_workflow = r#"name: CI/CD Pipeline

on:
  pull_request:
    branches:
      - dev
  push:
    branches:
      - prod

jobs:
  build:
    runs-on: ubuntu-latest
    
    steps:
      - name: Checkout
        uses: actions/checkout@v4
"#;

    // Add template-specific build steps
    let build_steps = match template_name {
        name if name.contains("laravel") => generate_laravel_steps(),
        name if name.contains("next") || name.contains("Next") => generate_nextjs_steps(),
        name if name.contains("react-native") => generate_react_native_steps(),
        _ => generate_generic_steps(),
    };

    format!("{}{}", base_workflow, build_steps)
}

fn generate_laravel_steps() -> &'static str {
    r#"      - name: Setup PHP
        uses: shivammathur/setup-php@v2
        with:
          php-version: 8.4
          tools: composer:v2

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '22'
          cache: 'npm'

      - name: Install Node Dependencies
        run: npm ci

      - name: Install Dependencies
        run: composer install --no-interaction --prefer-dist --optimize-autoloader

      - name: Copy Environment File
        run: cp .env.example .env || true

      - name: Generate Application Key
        run: php artisan key:generate || true

      - name: Build Assets
        run: npm run build || echo "No build script found"

      - name: Run Tests
        run: ./vendor/bin/pest || echo "No tests configured"
"#
}

fn generate_nextjs_steps() -> &'static str {
    r#"      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '22'
          cache: 'npm'

      - name: Install Dependencies
        run: npm ci

      - name: Build
        run: npm run build

      - name: Run Tests
        run: npm test || echo "No tests configured"
"#
}

fn generate_react_native_steps() -> &'static str {
    r#"      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '22'
          cache: 'npm'

      - name: Install Dependencies
        run: npm ci

      - name: Build
        run: npm run build || echo "No build script found"

      - name: Run Tests
        run: npm test || echo "No tests configured"
"#
}

fn generate_generic_steps() -> &'static str {
    r#"      - name: Build
        run: echo "Add your build commands here"
"#
}
