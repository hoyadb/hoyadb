require 'json'
package = JSON.parse(File.read(File.join(__dir__, 'package.json')))
Pod::Spec.new do |s|
  s.name = 'HoyaDBReactNative'
  s.version = package['version']
  s.summary = 'Owned-byte HoyaDB TurboModule over the general Rust facade'
  s.homepage = 'https://github.com/hoyadb/hoyadb'
  s.license = { :type => 'MIT', :file => 'LICENSE' }
  s.author = { 'HoyaDB' => 'contact@btk-consulting.fr' }
  s.source = { :git => 'https://github.com/hoyadb/hoyadb.git', :tag => "v#{s.version}" }
  s.platforms = { :ios => '15.1' }
  s.source_files = 'ios/**/*.{h,mm}'
  s.vendored_frameworks = 'Artifacts/HoyaNative.xcframework'
  s.frameworks = 'Security', 'CoreFoundation'
  s.pod_target_xcconfig = { 'CLANG_CXX_LANGUAGE_STANDARD' => 'c++20' }
  install_modules_dependencies(s)
end
