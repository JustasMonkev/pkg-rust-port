'use strict';

try {
  require.resolve('reqResSomeLit');
} catch (_) {
  // Missing modules are expected; this fixture exercises exclusion diagnostics.
}
try {
  require.resolve('reqResSomeLitMay', 'may-exclude');
} catch (_) {
  // Missing modules are expected; this fixture exercises exclusion diagnostics.
}
try {
  require.resolve('reqResSomeLitMust', 'must-exclude');
} catch (_) {
  // Missing modules are expected; this fixture exercises exclusion diagnostics.
}
try {
  require('reqSomeLit');
} catch (_) {
  // Missing modules are expected; this fixture exercises exclusion diagnostics.
}
try {
  require('reqSomeLitMay', 'may-exclude');
} catch (_) {
  // Missing modules are expected; this fixture exercises exclusion diagnostics.
}
try {
  require('reqSomeLitMust', 'must-exclude');
} catch (_) {
  // Missing modules are expected; this fixture exercises exclusion diagnostics.
}

var tryReqResSomeVar = 'some';
var tryReqResSomeVarMay = 'some';
var tryReqResSomeVarMust = 'some';

var tryReqSomeVar = 'some';
var tryReqSomeVarMay = 'some';
var tryReqSomeVarMust = 'some';

try {
  require.resolve(tryReqResSomeVar);
} catch (_) {
  // Missing modules are expected; this fixture exercises exclusion diagnostics.
}
try {
  require.resolve(tryReqResSomeVarMay, 'may-exclude');
} catch (_) {
  // Missing modules are expected; this fixture exercises exclusion diagnostics.
}
try {
  require.resolve(tryReqResSomeVarMust, 'must-exclude');
} catch (_) {
  // Missing modules are expected; this fixture exercises exclusion diagnostics.
}
try {
  require(tryReqSomeVar);
} catch (_) {
  // Missing modules are expected; this fixture exercises exclusion diagnostics.
}
try {
  require(tryReqSomeVarMay, 'may-exclude');
} catch (_) {
  // Missing modules are expected; this fixture exercises exclusion diagnostics.
}
try {
  require(tryReqSomeVarMust, 'must-exclude');
} catch (_) {
  // Missing modules are expected; this fixture exercises exclusion diagnostics.
}

var reqResSomeVar = 'some';
var reqResSomeVarMay = 'some';
var reqResSomeVarMust = 'some';

var reqSomeVar = 'some';
var reqSomeVarMay = 'some';
var reqSomeVarMust = 'some';

require.resolve(reqResSomeVar);
require.resolve(reqResSomeVarMay, 'may-exclude');
require.resolve(reqResSomeVarMust, 'must-exclude');
require.resolve(reqResSomeVar, reqResSomeVar);
require.resolve(reqResSomeVar, 'can-can');
require(reqSomeVar);
require(reqSomeVarMay, 'may-exclude');
require(reqSomeVarMust, 'must-exclude');
require(reqSomeVar, reqSomeVar);
require(reqSomeVar, 'can-can');

require.resolve('./test-y-index.js');
require('./test-y-index.js');
