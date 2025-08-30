#!/usr/bin/env node

import { execSync, spawn } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

// ES Module compatibility
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Load configuration
const configPath = path.resolve(__dirname, './config.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf-8'));

const CLUSTER = config.network.cluster;

// Deployment phases
const PHASES = [
    {
        name: 'Token Deployment',
        script: 'init_mdoge_token.js',
        description: 'Deploy mDOGE token with Token 2022 extensions',
        required: true,
        estimatedTime: '2-3 minutes'
    },
    {
        name: 'Pool Configuration',
        script: 'init_mdoge_SOL_pool.js',
        description: 'Configure Raydium pool parameters',
        required: false,
        estimatedTime: '1-2 minutes'
    },
    {
        name: 'MoonBase Initialization',
        script: 'init_moonBase.js',
        description: 'Initialize complete MoonBase gaming system',
        required: true,
        estimatedTime: '5-10 minutes'
    }
];

// ============================================================================
// ========== MAIN ORCHESTRATION SCRIPT =====================================
// ============================================================================

async function main() {
    try {
        console.log('\x1b[35m%s\x1b[0m', '🚀 ================================ DogeTech Master Deployment ================================');
        console.log('\x1b[36m%s\x1b[0m', '🌐 Target Network:', CLUSTER);
        console.log('\x1b[36m%s\x1b[0m', '⏱️  Estimated Total Time: 8-15 minutes');
        console.log('\x1b[36m%s\x1b[0m', '📋 Phases to Execute:', PHASES.length);
        
        // Parse command line arguments
        const args = process.argv.slice(2);
        const options = parseArguments(args);
        
        // Show help if requested
        if (options.help) {
            showHelp();
            return;
        }
        
        // Validate prerequisites
        await validatePrerequisites();
        
        // Show deployment plan
        showDeploymentPlan(options);
        
        // Wait for user confirmation unless auto mode
        if (!options.auto) {
            await waitForConfirmation();
        }
        
        // Execute deployment phases
        let completedPhases = 0;
        const startTime = Date.now();
        
        for (let i = 0; i < PHASES.length; i++) {
            const phase = PHASES[i];
            
            // Skip phase if not included in options
            if (options.phases && !options.phases.includes(i + 1)) {
                console.log('\x1b[33m%s\x1b[0m', `⏭️  Skipping Phase ${i + 1}: ${phase.name}`);
                continue;
            }
            
            try {
                console.log('\x1b[35m%s\x1b[0m', `\n🔄 ================================ Phase ${i + 1}/${PHASES.length}: ${phase.name} ================================`);
                console.log('\x1b[36m%s\x1b[0m', `📝 ${phase.description}`);
                console.log('\x1b[36m%s\x1b[0m', `⏱️  Estimated Time: ${phase.estimatedTime}`);
                
                await executePhase(phase, options);
                
                completedPhases++;
                console.log('\x1b[32m%s\x1b[0m', `✅ Phase ${i + 1} completed successfully!`);
                
                // Progress update
                const progress = Math.round((completedPhases / PHASES.length) * 100);
                console.log('\x1b[90m%s\x1b[0m', `📊 Overall Progress: ${progress}% (${completedPhases}/${PHASES.length} phases)`);
                
            } catch (error) {
                console.error('\x1b[31m%s\x1b[0m', `❌ Phase ${i + 1} failed:`, error.message);
                
                if (phase.required) {
                    console.error('\x1b[31m%s\x1b[0m', '❌ This phase is required. Deployment cannot continue.');
                    process.exit(1);
                } else {
                    console.log('\x1b[33m%s\x1b[0m', '⚠️ This phase is optional. Continuing with next phase...');
                }
            }
        }
        
        // Final summary
        const endTime = Date.now();
        const totalTime = Math.round((endTime - startTime) / 1000);
        
        console.log('\x1b[35m%s\x1b[0m', '\n🎉 ================================ DEPLOYMENT COMPLETE ================================');
        console.log('\x1b[32m%s\x1b[0m', `✅ Successfully completed ${completedPhases}/${PHASES.length} phases`);
        console.log('\x1b[36m%s\x1b[0m', `⏱️  Total Deployment Time: ${Math.floor(totalTime / 60)}m ${totalTime % 60}s`);
        console.log('\x1b[36m%s\x1b[0m', `🌐 Network: ${CLUSTER}`);
        
        // Show next steps
        showNextSteps();
        
    } catch (error) {
        console.error('\x1b[31m%s\x1b[0m', '❌ Deployment orchestration failed:', error.message);
        process.exit(1);
    }
}

// ============================================================================
// ========== HELPER FUNCTIONS ===============================================
// ============================================================================

function parseArguments(args) {
    const options = {
        auto: false,
        phases: null,
        help: false,
        verbose: false
    };
    
    for (let i = 0; i < args.length; i++) {
        switch (args[i]) {
            case '--auto':
            case '-a':
                options.auto = true;
                break;
            case '--phases':
            case '-p':
                if (i + 1 < args.length) {
                    options.phases = args[i + 1].split(',').map(n => parseInt(n.trim()));
                    i++;
                } else {
                    throw new Error('--phases requires comma-separated phase numbers');
                }
                break;
            case '--help':
            case '-h':
                options.help = true;
                break;
            case '--verbose':
            case '-v':
                options.verbose = true;
                break;
            default:
                throw new Error(`Unknown argument: ${args[i]}`);
        }
    }
    
    return options;
}

function showHelp() {
    console.log(`
🚀 DogeTech Master Deployment Script

Usage: node deploy.js [options]

Options:
  --auto, -a              Run without confirmation prompts
  --phases, -p <numbers>  Run specific phases only (e.g., --phases 1,3)
  --verbose, -v           Enable verbose logging
  --help, -h              Show this help message

Phases:
  1. Token Deployment     Deploy mDOGE token (required)
  2. Pool Configuration   Configure Raydium pool (optional)
  3. MoonBase Init        Initialize game system (required)

Examples:
  node deploy.js                    # Full deployment with prompts
  node deploy.js --auto             # Automated full deployment
  node deploy.js --phases 1,3       # Deploy only token and moonbase
  node deploy.js --auto --verbose   # Automated with verbose logging

Environment:
  Network: ${CLUSTER}
  Config:  ./config.json
  State:   ./deployments/${CLUSTER}.json
    `);
}

async function validatePrerequisites() {
    console.log('\x1b[33m%s\x1b[0m', '🔍 Validating deployment prerequisites...');
    
    // Check Node.js version
    const nodeVersion = process.version;
    console.log('\x1b[36m%s\x1b[0m', `📦 Node.js Version: ${nodeVersion}`);
    
    // Check if config file exists
    if (!fs.existsSync(configPath)) {
        throw new Error('Configuration file not found: ./config.json');
    }
    console.log('\x1b[32m%s\x1b[0m', '✅ Configuration file found');
    
    // Check if required scripts exist
    for (const phase of PHASES) {
        const scriptPath = path.resolve(__dirname, phase.script);
        if (!fs.existsSync(scriptPath)) {
            throw new Error(`Script not found: ${phase.script}`);
        }
    }
    console.log('\x1b[32m%s\x1b[0m', '✅ All deployment scripts found');
    
    // Check npm dependencies
    try {
        execSync('npm list @solana/web3.js @solana/spl-token @coral-xyz/anchor', { 
            stdio: 'pipe',
            cwd: __dirname 
        });
        console.log('\x1b[32m%s\x1b[0m', '✅ Required dependencies installed');
    } catch (error) {
        console.log('\x1b[33m%s\x1b[0m', '⚠️ Some dependencies may be missing. Installing...');
        try {
            execSync('npm install @solana/web3.js @solana/spl-token @coral-xyz/anchor @metaplex-foundation/mpl-token-metadata', {
                stdio: 'inherit',
                cwd: __dirname
            });
            console.log('\x1b[32m%s\x1b[0m', '✅ Dependencies installed successfully');
        } catch (installError) {
            throw new Error('Failed to install required dependencies');
        }
    }
    
    // Check Anchor CLI (optional)
    try {
        execSync('anchor --version', { stdio: 'pipe' });
        console.log('\x1b[32m%s\x1b[0m', '✅ Anchor CLI available');
    } catch (error) {
        console.log('\x1b[33m%s\x1b[0m', '⚠️ Anchor CLI not found (optional for deployment scripts)');
    }
    
    console.log('\x1b[32m%s\x1b[0m', '✅ All prerequisites validated');
}

function showDeploymentPlan(options) {
    console.log('\x1b[33m%s\x1b[0m', '\n📋 Deployment Plan:');
    
    for (let i = 0; i < PHASES.length; i++) {
        const phase = PHASES[i];
        const willExecute = !options.phases || options.phases.includes(i + 1);
        const status = willExecute ? '✅ Execute' : '⏭️ Skip';
        const required = phase.required ? '[Required]' : '[Optional]';
        
        console.log('\x1b[36m%s\x1b[0m', `   ${i + 1}. ${phase.name} ${required} - ${status}`);
        console.log('\x1b[90m%s\x1b[0m', `      ${phase.description}`);
        console.log('\x1b[90m%s\x1b[0m', `      Estimated: ${phase.estimatedTime}`);
    }
    
    console.log('\x1b[36m%s\x1b[0m', `\n🌐 Target Network: ${CLUSTER}`);
    console.log('\x1b[36m%s\x1b[0m', `📁 State File: ./deployments/${CLUSTER}.json`);
}

async function waitForConfirmation() {
    console.log('\x1b[33m%s\x1b[0m', '\n⚠️ Ready to begin deployment. This will:');
    console.log('\x1b[33m%s\x1b[0m', '   • Create/modify blockchain accounts');
    console.log('\x1b[33m%s\x1b[0m', '   • Spend SOL for transaction fees');
    console.log('\x1b[33m%s\x1b[0m', '   • Initialize game contracts');
    console.log('\x1b[33m%s\x1b[0m', '   • Deploy tokens to the network');
    
    // Simple confirmation prompt
    const readline = await import('readline');
    const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout
    });
    
    return new Promise((resolve) => {
        rl.question('\n🤔 Do you want to continue? (y/N): ', (answer) => {
            rl.close();
            if (answer.toLowerCase() === 'y' || answer.toLowerCase() === 'yes') {
                console.log('\x1b[32m%s\x1b[0m', '✅ Deployment confirmed. Starting...');
                resolve();
            } else {
                console.log('\x1b[33m%s\x1b[0m', '❌ Deployment cancelled by user.');
                process.exit(0);
            }
        });
    });
}

async function executePhase(phase, options) {
    const scriptPath = path.resolve(__dirname, phase.script);
    
    return new Promise((resolve, reject) => {
        console.log('\x1b[36m%s\x1b[0m', `🔄 Executing: node ${phase.script}`);
        
        const child = spawn('node', [scriptPath], {
            cwd: __dirname,
            stdio: options.verbose ? 'inherit' : 'pipe',
            env: {
                ...process.env,
                DEPLOYMENT_PHASE: phase.name
            }
        });
        
        let output = '';
        let errorOutput = '';
        
        if (!options.verbose) {
            child.stdout?.on('data', (data) => {
                output += data.toString();
                // Show progress indicators
                const lines = data.toString().split('\n');
                for (const line of lines) {
                    if (line.includes('✅') || line.includes('❌') || line.includes('⚠️')) {
                        console.log(line);
                    }
                }
            });
            
            child.stderr?.on('data', (data) => {
                errorOutput += data.toString();
                console.error('\x1b[31m%s\x1b[0m', data.toString());
            });
        }
        
        child.on('close', (code) => {
            if (code === 0) {
                resolve();
            } else {
                const error = new Error(`Script exited with code ${code}`);
                error.output = output;
                error.errorOutput = errorOutput;
                reject(error);
            }
        });
        
        child.on('error', (error) => {
            reject(new Error(`Failed to execute script: ${error.message}`));
        });
    });
}

function showNextSteps() {
    console.log('\x1b[36m%s\x1b[0m', '\n📋 Next Steps:');
    console.log('\x1b[36m%s\x1b[0m', '   1. 🔍 Verify deployment in Solana Explorer');
    console.log('\x1b[36m%s\x1b[0m', '   2. 🧪 Test basic functionality');
    console.log('\x1b[36m%s\x1b[0m', '   3. 🚀 Deploy frontend application');
    console.log('\x1b[36m%s\x1b[0m', '   4. 📊 Set up monitoring and analytics');
    console.log('\x1b[36m%s\x1b[0m', '   5. 🎮 Begin user testing');
    
    console.log('\x1b[90m%s\x1b[0m', '\n📁 Important Files:');
    console.log('\x1b[90m%s\x1b[0m', `   • Deployment State: ./deployments/${CLUSTER}.json`);
    console.log('\x1b[90m%s\x1b[0m', `   • Configuration: ./config.json`);
    
    console.log('\x1b[90m%s\x1b[0m', '\n🔗 Useful Commands:');
    console.log('\x1b[90m%s\x1b[0m', '   • solana config set --url devnet');
    console.log('\x1b[90m%s\x1b[0m', '   • solana balance');
    console.log('\x1b[90m%s\x1b[0m', '   • anchor test');
    
    console.log('\x1b[35m%s\x1b[0m', '\n🌙 Welcome to the Moon! DogeTech deployment complete. 🚀');
}

// Handle process termination gracefully
process.on('SIGINT', () => {
    console.log('\x1b[33m%s\x1b[0m', '\n⚠️ Deployment interrupted by user.');
    console.log('\x1b[33m%s\x1b[0m', '💾 Deployment state has been preserved.');
    console.log('\x1b[33m%s\x1b[0m', '🔄 You can resume by running the script again.');
    process.exit(0);
});

process.on('uncaughtException', (error) => {
    console.error('\x1b[31m%s\x1b[0m', '💥 Uncaught exception:', error.message);
    console.error('\x1b[31m%s\x1b[0m', '🔍 Check the logs and state files for debugging.');
    process.exit(1);
});

// Run the main function
main().catch((error) => {
    console.error('\x1b[31m%s\x1b[0m', '💥 Deployment failed:', error.message);
    process.exit(1);
}); 