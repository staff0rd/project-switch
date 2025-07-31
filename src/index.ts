#!/usr/bin/env node

import { Command } from 'commander';
import chalk from 'chalk';
import { switchCommand } from './commands/switch';
import { addCommand } from './commands/add';
import { openCommand } from './commands/open';
import { ConfigManager } from './config';

const program = new Command();

program
  .name('project-switch')
  .description('CLI tool to manage and switch between projects')
  .version('1.0.0');

program
  .command('switch')
  .description('Switch between projects')
  .action(async () => {
    try {
      await switchCommand();
    } catch (error: any) {
      console.error(chalk.red('Error:'), error instanceof Error ? error.message : 'Unknown error');
      process.exit(1);
    }
  });

program
  .command('add [name]')
  .description('Add a new project')
  .action(async (name?: string) => {
    try {
      await addCommand(name);
    } catch (error: any) {
      console.error(chalk.red('Error:'), error instanceof Error ? error.message : 'Unknown error');
      process.exit(1);
    }
  });

program
  .command('current')
  .description('Show the current project')
  .action(() => {
    try {
      const configManager = new ConfigManager();
      const currentProject = configManager.getCurrentProject();
      if (currentProject) {
        console.log(chalk.green(`Current project: ${currentProject}`));
      } else {
        console.log(chalk.yellow('No current project selected'));
      }
    } catch (error: any) {
      console.error(chalk.red('Error:'), error instanceof Error ? error.message : 'Unknown error');
      process.exit(1);
    }
  });

program
  .command('open <key>')
  .description('Open a URL associated with the current project')
  .action(async (key: string) => {
    try {
      await openCommand(key);
    } catch (error: any) {
      console.error(chalk.red('Error:'), error instanceof Error ? error.message : 'Unknown error');
      process.exit(1);
    }
  });

program.parse();