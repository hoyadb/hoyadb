# Owned ordinary consumer project, generated with the installed xcodeproj tool.
# Run only after the restricted activation plan's archive/tool checks pass.
require 'pathname'
require 'xcodeproj'

consumer = Pathname.new(ARGV.fetch(0)).realpath
swift_package = Pathname.new(ARGV.fetch(1)).realpath
ios = consumer.join('ios')
destination = ios.join('HoyaConsumer.xcodeproj')
abort 'owned project already exists' if destination.exist?
abort 'consumer sources missing' unless ios.join('ForeignControl.swift').file?
abort 'public Swift package missing' unless swift_package.join('Package.swift').file?

project = Xcodeproj::Project.new(destination.to_s)
target = project.new_target(:application, 'HoyaConsumer', :ios, '15.1')
group = project.main_group.new_group('Consumer', '.', '<group>')
%w[main.mm AppDelegate.mm ForeignControl.swift HoyaConsumerModule.mm].each do |name|
  target.source_build_phase.add_file_reference(group.new_file(name))
end
%w[AppDelegate.h HoyaConsumerModule.h Info.plist].each { |name| group.new_file(name) }
target.resources_build_phase.add_file_reference(group.new_file('../main.jsbundle'))

reference = project.new(Xcodeproj::Project::Object::XCLocalSwiftPackageReference)
reference.relative_path = swift_package.to_s
project.root_object.package_references << reference
product = project.new(Xcodeproj::Project::Object::XCSwiftPackageProductDependency)
product.package = reference
product.product_name = 'HoyaDB'
target.package_product_dependencies << product
link = project.new(Xcodeproj::Project::Object::PBXBuildFile)
link.product_ref = product
target.frameworks_build_phase.files << link

target.build_configurations.each do |configuration|
  configuration.build_settings.merge!({
    'PRODUCT_BUNDLE_IDENTIFIER' => 'dev.hoyadb.phase7.rn.consumer',
    'PRODUCT_NAME' => 'HoyaConsumer',
    'PRODUCT_MODULE_NAME' => 'HoyaConsumer',
    'INFOPLIST_FILE' => 'Info.plist',
    'SWIFT_VERSION' => '6.0',
    'SWIFT_OBJC_INTERFACE_HEADER_NAME' => 'HoyaConsumer-Swift.h',
    'SWIFT_INSTALL_OBJC_HEADER' => 'YES',
    'CLANG_ENABLE_MODULES' => 'YES',
    'CLANG_CXX_LANGUAGE_STANDARD' => 'c++20',
    'CODE_SIGNING_ALLOWED' => 'NO',
    'CODE_SIGNING_REQUIRED' => 'NO',
    'ENABLE_USER_SCRIPT_SANDBOXING' => 'NO',
    'TARGETED_DEVICE_FAMILY' => '1,2',
    'GENERATE_INFOPLIST_FILE' => 'NO'
  })
end
project.save
scheme = Xcodeproj::XCScheme.new
scheme.add_build_target(target)
scheme.set_launch_target(target)
scheme.save_as(project.path, 'HoyaConsumer', true)
