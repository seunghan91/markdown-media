class AddMissingColumnsToTasks < ActiveRecord::Migration[7.0]
  def change
    # deleted_at 컬럼이 없으면 추가
    unless column_exists?(:tasks, :deleted_at)
      add_column :tasks, :deleted_at, :datetime
      add_index :tasks, :deleted_at
    end
    
    # completed_at 컬럼이 없으면 추가
    unless column_exists?(:tasks, :completed_at)
      add_column :tasks, :completed_at, :datetime
      add_index :tasks, :completed_at
    end
    
    # version 컬럼이 없으면 추가
    unless column_exists?(:tasks, :version)
      add_column :tasks, :version, :integer, default: 1
    end
    
    # is_important 컬럼이 없으면 추가
    unless column_exists?(:tasks, :is_important)
      add_column :tasks, :is_important, :boolean, default: false
    end
    
    # has_specific_time 컬럼이 없으면 추가
    unless column_exists?(:tasks, :has_specific_time)
      add_column :tasks, :has_specific_time, :boolean, default: false
    end
    
    # is_all_day 컬럼이 없으면 추가
    unless column_exists?(:tasks, :is_all_day)
      add_column :tasks, :is_all_day, :boolean, default: false
    end
  end
end