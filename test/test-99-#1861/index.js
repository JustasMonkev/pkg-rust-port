'use strict';

// Regression fixture for issue #1861: a packaged Windows executable must be
// able to relaunch itself as a child process. The parent prints "launch" and
// then spawns a copy of itself with the "stop" command, whose output is piped
// back through so the combined stdout contains both "launch" and "stop".

const { execFileSync } = require('child_process');

const command = process.argv[2];

if (command === 'launch') {
  console.log('launch');
  const child = execFileSync(process.execPath, ['stop']);
  process.stdout.write(child.toString());
} else if (command === 'stop') {
  console.log('stop');
} else {
  console.log(command);
}
