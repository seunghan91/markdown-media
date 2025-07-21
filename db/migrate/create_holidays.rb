class CreateHolidays < ActiveRecord::Migration[7.0]
  def change
    create_table :holidays do |t|
      t.date :date, null: false
      t.string :name, null: false
      t.string :country, default: 'KR'
      t.boolean :is_substitute, default: false
      t.text :description

      t.timestamps
    end

    add_index :holidays, [:date, :country], unique: true
    add_index :holidays, :date
    add_index :holidays, :country
  end
end