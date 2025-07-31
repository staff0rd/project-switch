import { ConfigManager } from '../config';
import chalk from 'chalk';
import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

export async function openCommand(key: string): Promise<void> {
  const configManager = new ConfigManager();
  const currentProjectName = configManager.getCurrentProject();

  if (!currentProjectName) {
    console.error(chalk.red('Error: No current project selected'));
    console.log(chalk.yellow('Use "project-switch switch" to select a project first'));
    return;
  }

  const project = configManager.getProject(currentProjectName);
  const command = configManager.getProjectCommand(currentProjectName, key);

  if (!command) {
    console.error(chalk.red(`Error: Command with key "${key}" not found in project "${currentProjectName}"`));
    return;
  }

  if (!command.url) {
    console.error(chalk.red(`Error: Command "${key}" does not have a URL configured`));
    return;
  }

  // Browser hierarchy: command > project > config > default
  const browser = command.browser || project?.browser || configManager.getDefaultBrowser();
  
  try {
    let cmd: string;
    
    // Handle different operating systems
    if (process.platform === 'win32') {
      if (browser.toLowerCase() === 'default') {
        cmd = `start "" "${command.url}"`;
      } else {
        cmd = `start ${browser} "${command.url}"`;
      }
    } else if (process.platform === 'darwin') {
      if (browser.toLowerCase() === 'default') {
        cmd = `open "${command.url}"`;
      } else {
        cmd = `open -a "${browser}" "${command.url}"`;
      }
    } else {
      // Linux/Unix
      if (browser.toLowerCase() === 'default') {
        cmd = `xdg-open "${command.url}"`;
      } else {
        cmd = `${browser} "${command.url}"`;
      }
    }

    console.log(chalk.green(`Opening ${command.url} in ${browser}...`));
    await execAsync(cmd);
  } catch (error) {
    console.error(chalk.red('Error opening URL:'), error instanceof Error ? error.message : 'Unknown error');
  }
}