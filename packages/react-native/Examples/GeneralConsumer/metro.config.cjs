const fs=require('fs');
const path=require('path');
const {getDefaultConfig,mergeConfig}=require('@react-native/metro-config');
// Metro crawls only the project and watchFolders. A consumer whose node_modules entries link to a
// read-only verified install must watch those real roots; an ordinary install adds nothing here.
const modules=path.join(__dirname,'node_modules');
const local=fs.realpathSync(modules);
const linked=[...new Set(fs.readdirSync(modules).map(name=>path.dirname(fs.realpathSync(path.join(modules,name)))))].filter(root=>root!==local);
module.exports=mergeConfig(getDefaultConfig(__dirname),{watchFolders:linked,resolver:{unstable_enablePackageExports:true}});
